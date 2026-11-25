//! Tests de integración con repos reales en directorios temporales (si no hay `git`, retornan).

use std::path::{Path, PathBuf};
use std::process::Command;

use super::diff::{hunk_patch, DiffTarget};
use super::log::LogQuery;
use super::ops::{self, BranchAction, ConflictAction, ResetMode, SeqAction, SeqOp, StashAction, TagAction};
use super::remote::{self, GitProgress, RemoteOp};
use super::status2::RepoState;
use super::{blame, diff, log, refs, status2};
use crate::state::AppState;
use crate::util::paths::clean_canonicalize;

struct TestRepo {
    _dir: tempfile::TempDir,
    root: PathBuf,
    state: AppState,
}

fn have_git() -> bool {
    let ok = Command::new("git").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    if !ok {
        eprintln!("git no está en PATH: test omitido");
    }
    ok
}

fn git_sync(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(args).current_dir(dir).output().expect("git");
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn setup_repo(dir: &Path) {
    git_sync(dir, &["init", "-q", "-b", "main"]);
    git_sync(dir, &["config", "user.name", "Test"]);
    git_sync(dir, &["config", "user.email", "t@example.com"]);
    git_sync(dir, &["config", "commit.gpgsign", "false"]);
    git_sync(dir, &["config", "core.autocrlf", "false"]);
}

impl TestRepo {
    fn new() -> Option<Self> {
        if !have_git() {
            return None;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = clean_canonicalize(dir.path()).unwrap();
        setup_repo(&root);
        let state = AppState::new();
        state.workspace.write().unwrap().replace(crate::workspace::Workspace::single(root.clone()));
        Some(Self { _dir: dir, root, state })
    }
    fn write(&self, rel: &str, content: &str) {
        let p = self.root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }
    fn g(&self, args: &[&str]) -> String {
        git_sync(&self.root, args)
    }
    /// Escribe, añade y hace commit; devuelve el sha.
    fn commit_file(&self, rel: &str, content: &str, msg: &str) -> String {
        self.write(rel, content);
        self.g(&["add", "--", rel]);
        self.g(&["commit", "-q", "-m", msg]);
        self.g(&["rev-parse", "HEAD"])
    }
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[tokio::test]
async fn status_v2_real_repo() {
    let Some(r) = TestRepo::new() else { return };
    r.commit_file("a.txt", "1\n", "init");
    r.commit_file("ren ame.txt", "x\ny\nz\n", "second");
    r.write("a.txt", "1\n2\n");
    r.write("staged.txt", "s\n");
    r.g(&["add", "staged.txt"]);
    r.write("dir/ñandú nuevo.txt", "n\n");
    r.g(&["mv", "ren ame.txt", "renombrado ñ.txt"]);
    let st = status2::status_v2(&r.state, None).await.unwrap();
    assert!(st.info.is_repo);
    assert_eq!(st.info.branch.as_deref(), Some("main"));
    assert_eq!(st.info.state, RepoState::Clean);
    assert!(st.info.git_version.is_some());
    assert!(st.unstaged.iter().any(|e| e.path == "a.txt" && e.worktree == 'M'));
    assert!(st.staged.iter().any(|e| e.path == "staged.txt" && e.index == 'A'));
    let ren = st.staged.iter().find(|e| e.path == "renombrado ñ.txt").expect("rename");
    assert_eq!(ren.orig_path.as_deref(), Some("ren ame.txt"));
    assert!(st.untracked.iter().any(|e| e.path == "dir/ñandú nuevo.txt"));
    let info = status2::repo_info(&r.state, None).await.unwrap();
    assert!(info.head.is_some());
    assert_eq!(info.top.as_deref(), Some(r.root.to_string_lossy().as_ref()));
}

#[tokio::test]
async fn status_outside_repo_is_not_repo() {
    if !have_git() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let st = AppState::new();
    st.workspace.write().unwrap().replace(crate::workspace::Workspace::single(clean_canonicalize(d.path()).unwrap()));
    let s = status2::status_v2(&st, None).await.unwrap();
    assert!(!s.info.is_repo);
    // git_init crea el repo con rama main.
    let o = ops::init(&st, d.path().to_string_lossy().into_owned()).await.unwrap();
    assert!(o.ok);
    let s = status2::status_v2(&st, None).await.unwrap();
    assert!(s.info.is_repo);
    assert_eq!(s.info.branch.as_deref(), Some("main"));
    assert!(ops::init(&st, d.path().to_string_lossy().into_owned()).await.is_err());
}

#[tokio::test]
async fn log_pagination_merge_and_refs() {
    let Some(r) = TestRepo::new() else { return };
    // repo vacío: log vacío sin error
    let empty = log::log(&r.state, None, LogQuery { skip: 0, limit: 10, ..Default::default() }).await.unwrap();
    assert!(empty.commits.is_empty());
    r.commit_file("a.txt", "a\n", "c1");
    r.g(&["switch", "-q", "-c", "feature/x"]);
    r.commit_file("b.txt", "b\n", "c2 feature");
    r.g(&["switch", "-q", "main"]);
    r.commit_file("c.txt", "c\n", "c3 main");
    r.g(&["merge", "-q", "--no-ff", "-m", "merge feature", "feature/x"]);
    r.g(&["tag", "light"]);
    r.g(&["tag", "-a", "ann", "-m", "annotated msg"]);
    let page = log::log(&r.state, None, LogQuery { skip: 0, limit: 2, all: true, ..Default::default() }).await.unwrap();
    assert_eq!(page.commits.len(), 2);
    assert!(page.has_more);
    let head = &page.commits[0];
    assert_eq!(head.subject, "merge feature");
    assert_eq!(head.parents.len(), 2);
    assert!(head.refs.iter().any(|l| l.name == "main"));
    assert!(head.refs.iter().any(|l| l.name == "light"));
    let all = log::log(&r.state, None, LogQuery { skip: 0, limit: 50, all: true, ..Default::default() }).await.unwrap();
    assert_eq!(all.commits.len(), 4);
    assert!(!all.has_more);
    let rest = log::log(&r.state, None, LogQuery { skip: 2, limit: 50, ..Default::default() }).await.unwrap();
    assert_eq!(rest.commits.len(), 2);
    let by_path = log::log(&r.state, None, LogQuery { skip: 0, limit: 50, path: Some("b.txt".into()), ..Default::default() }).await.unwrap();
    assert_eq!(by_path.commits.len(), 1);
    let found = log::log(&r.state, None, LogQuery { skip: 0, limit: 50, search: Some("FEATURE".into()), ..Default::default() }).await.unwrap();
    assert_eq!(found.commits.len(), 2);

    let rf = refs::refs(&r.state, None).await.unwrap();
    assert!(rf.local.iter().any(|b| b.name == "main" && b.is_head));
    assert!(rf.local.iter().any(|b| b.name == "feature/x"));
    let ann = rf.tags.iter().find(|t| t.name == "ann").unwrap();
    assert!(ann.annotated);
    assert_eq!(ann.message.as_deref(), Some("annotated msg"));
    assert!(!rf.tags.iter().find(|t| t.name == "light").unwrap().annotated);

    let d = log::commit_detail(&r.state, None, head.sha.clone()).await.unwrap();
    assert_eq!(d.row.subject, "merge feature");
    assert_eq!(d.committer, "Test");
    assert!(d.files.iter().any(|f| f.path == "b.txt" && f.status == 'A' && f.additions == Some(1)));
    let first = all.commits.last().unwrap();
    let d0 = log::commit_detail(&r.state, None, first.sha.clone()).await.unwrap();
    assert_eq!(d0.files.len(), 1);
}

#[tokio::test]
async fn refs_with_bare_remote_ahead_behind_and_stash() {
    let Some(r) = TestRepo::new() else { return };
    let bare = tempfile::tempdir().unwrap();
    git_sync(bare.path(), &["init", "-q", "--bare", "-b", "main"]);
    let bare_path = bare.path().to_string_lossy().into_owned();
    r.commit_file("a.txt", "1\n", "c1");
    r.g(&["remote", "add", "origin", &bare_path]);
    r.g(&["push", "-q", "-u", "origin", "main"]);
    r.commit_file("a.txt", "2\n", "c2");
    r.write("a.txt", "wip\n");
    r.g(&["stash", "push", "-m", "mi stash"]);
    let rf = refs::refs(&r.state, None).await.unwrap();
    let main = rf.local.iter().find(|b| b.name == "main").unwrap();
    assert_eq!(main.upstream.as_deref(), Some("origin/main"));
    assert_eq!((main.ahead, main.behind), (1, 0));
    assert!(rf.remote.iter().any(|b| b.name == "origin/main"));
    assert_eq!(rf.remotes.len(), 1);
    assert_eq!(rf.stashes.len(), 1);
    assert!(rf.stashes[0].message.contains("mi stash"));
    let info = status2::repo_info(&r.state, None).await.unwrap();
    assert_eq!((info.ahead, info.behind), (1, 0));
    assert_eq!(info.upstream.as_deref(), Some("origin/main"));
}

fn two_hunk_content(a: &str, b: &str) -> String {
    let mut v: Vec<String> = (1..=30).map(|i| format!("line {i}")).collect();
    v[1] = a.into();
    v[27] = b.into();
    v.join("\n") + "\n"
}

#[tokio::test]
async fn hunk_stage_unstage_discard() {
    let Some(r) = TestRepo::new() else { return };
    r.commit_file("f.txt", &two_hunk_content("line 2", "line 28"), "base");
    r.write("f.txt", &two_hunk_content("CHANGED 2", "CHANGED 28"));
    let fd = diff::diff(&r.state, None, DiffTarget::Worktree { path: "f.txt".into() }, 3).await.unwrap();
    assert_eq!(fd.hunks.len(), 2);
    assert_eq!(fd.path, "f.txt");

    // stage solo el 2.º hunk
    let patch = hunk_patch(&fd, &[1]);
    let o = ops::apply_patch(&r.state, None, patch.clone(), true, false).await.unwrap();
    assert!(o.ok);
    let cached = diff::diff(&r.state, None, DiffTarget::Index { path: "f.txt".into() }, 3).await.unwrap();
    assert_eq!(cached.hunks.len(), 1);
    assert!(cached.hunks[0].lines.iter().any(|l| l.text == "CHANGED 28"));
    let wt = diff::diff(&r.state, None, DiffTarget::Worktree { path: "f.txt".into() }, 3).await.unwrap();
    assert_eq!(wt.hunks.len(), 1);
    assert!(wt.hunks[0].lines.iter().any(|l| l.text == "CHANGED 2"));

    // unstage (-R --cached)
    ops::apply_patch(&r.state, None, patch, true, true).await.unwrap();
    let cached = diff::diff(&r.state, None, DiffTarget::Index { path: "f.txt".into() }, 3).await.unwrap();
    assert!(cached.hunks.is_empty());

    // descartar el 1.er hunk del worktree
    let fd = diff::diff(&r.state, None, DiffTarget::Worktree { path: "f.txt".into() }, 3).await.unwrap();
    ops::apply_patch(&r.state, None, hunk_patch(&fd, &[0]), false, true).await.unwrap();
    let content = std::fs::read_to_string(r.root.join("f.txt")).unwrap();
    assert!(content.contains("line 2\n"));
    assert!(content.contains("CHANGED 28"));

    // diff de commit y rango
    let sha = r.g(&["rev-parse", "HEAD"]);
    let c = diff::diff(&r.state, None, DiffTarget::Commit { sha: sha.clone(), path: "f.txt".into() }, 3).await.unwrap();
    assert!(!c.hunks.is_empty()); // commit raíz
    let at = diff::file_at(&r.state, None, "HEAD".into(), "f.txt".into()).await.unwrap();
    assert!(at.contains("line 28\n"));
    let idx = diff::file_at(&r.state, None, "INDEX".into(), "f.txt".into()).await.unwrap();
    assert_eq!(idx, at);
    assert!(diff::file_at(&r.state, None, "-x".into(), "f.txt".into()).await.is_err());
}

#[tokio::test]
async fn untracked_diff_and_binary_file_at() {
    let Some(r) = TestRepo::new() else { return };
    r.commit_file("a.txt", "a\n", "c1");
    r.write("nuevo.txt", "uno\ndos\n");
    let fd = diff::diff(&r.state, None, DiffTarget::Worktree { path: "nuevo.txt".into() }, 3).await.unwrap();
    assert_eq!(fd.hunks.len(), 1);
    assert_eq!(fd.hunks[0].new_lines, 2);
    assert_eq!(fd.path, "nuevo.txt");
    std::fs::write(r.root.join("bin.dat"), [0u8, 1, 2, 0, 255]).unwrap();
    r.g(&["add", "bin.dat"]);
    r.g(&["commit", "-q", "-m", "bin"]);
    let e = diff::file_at(&r.state, None, "HEAD".into(), "bin.dat".into()).await.unwrap_err();
    assert!(e.to_string().contains("binario"));
}

#[tokio::test]
async fn stage_unstage_discard_paths() {
    let Some(r) = TestRepo::new() else { return };
    // sin commits: unstage usa rm --cached
    r.write("-guion.txt", "x\n");
    r.write("sub dir/b.txt", "b\n");
    ops::stage(&r.state, None, s(&["-guion.txt", "sub dir/b.txt"])).await.unwrap();
    let st = status2::status_v2(&r.state, None).await.unwrap();
    assert_eq!(st.staged.len(), 2);
    ops::unstage(&r.state, None, s(&["-guion.txt"])).await.unwrap();
    let st = status2::status_v2(&r.state, None).await.unwrap();
    assert_eq!(st.staged.len(), 1);
    assert!(st.untracked.iter().any(|e| e.path == "-guion.txt"));
    ops::commit(&r.state, None, "base".into(), false, false).await.unwrap();

    // unstage con HEAD
    r.write("sub dir/b.txt", "b2\n");
    ops::stage(&r.state, None, vec![r.root.join("sub dir/b.txt").to_string_lossy().into_owned()]).await.unwrap();
    ops::unstage(&r.state, None, s(&["sub dir/b.txt"])).await.unwrap();
    assert!(status2::status_v2(&r.state, None).await.unwrap().staged.is_empty());

    // discard tracked
    ops::discard(&r.state, None, s(&["sub dir/b.txt"]), false).await.unwrap();
    assert_eq!(std::fs::read_to_string(r.root.join("sub dir/b.txt")).unwrap(), "b\n");
    // discard de algo rastreado como "untracked" se rechaza
    assert!(ops::discard(&r.state, None, s(&["sub dir/b.txt"]), true).await.is_err());
    // rutas inválidas
    assert!(ops::stage(&r.state, None, s(&["../fuera"])).await.is_err());
    assert!(ops::stage(&r.state, None, vec![]).await.is_err());
}

#[tokio::test]
async fn commit_amend_and_multiline_message() {
    let Some(r) = TestRepo::new() else { return };
    r.write("a.txt", "a\n");
    ops::stage(&r.state, None, s(&["a.txt"])).await.unwrap();
    assert!(ops::commit(&r.state, None, "   ".into(), false, false).await.is_err());
    let msg = "Título \"con comillas\" y 'simples'\n\nCuerpo línea 1\n$(echo no) `x` \\n";
    ops::commit(&r.state, None, msg.into(), false, true).await.unwrap();
    let d = log::commit_detail(&r.state, None, "HEAD".into()).await.unwrap();
    assert_eq!(d.row.subject, "Título \"con comillas\" y 'simples'");
    assert!(d.body.contains("$(echo no) `x` \\n"));
    assert!(d.body.contains("Signed-off-by: Test"));
    // amend solo mensaje
    ops::commit(&r.state, None, "nuevo mensaje".into(), true, false).await.unwrap();
    let d = log::commit_detail(&r.state, None, "HEAD".into()).await.unwrap();
    assert_eq!(d.row.subject, "nuevo mensaje");
    // amend sin mensaje (--no-edit)
    r.write("b.txt", "b\n");
    ops::stage(&r.state, None, s(&["b.txt"])).await.unwrap();
    ops::commit(&r.state, None, String::new(), true, false).await.unwrap();
    let d = log::commit_detail(&r.state, None, "HEAD".into()).await.unwrap();
    assert_eq!(d.row.subject, "nuevo mensaje");
    assert_eq!(d.files.len(), 2);
    // nada que commitear
    assert!(ops::commit(&r.state, None, "x".into(), false, false).await.is_err());
    assert_eq!(r.state.git_busy_total(), 0);
}

#[tokio::test]
async fn branch_lifecycle() {
    let Some(r) = TestRepo::new() else { return };
    r.commit_file("a.txt", "a\n", "c1");
    let b = |a: BranchAction| ops::branch(&r.state, None, a);
    b(BranchAction::Create { name: "dev".into(), start: None, checkout: false }).await.unwrap();
    b(BranchAction::Rename { from: "dev".into(), to: "dev2".into() }).await.unwrap();
    b(BranchAction::Create { name: "feat/y".into(), start: Some("HEAD".into()), checkout: true }).await.unwrap();
    assert_eq!(status2::repo_info(&r.state, None).await.unwrap().branch.as_deref(), Some("feat/y"));
    r.commit_file("b.txt", "b\n", "solo en feat");
    b(BranchAction::Checkout { name: "main".into() }).await.unwrap();
    // no fusionada: -d falla, -D funciona
    assert!(b(BranchAction::Delete { name: "feat/y".into(), force: false }).await.is_err());
    b(BranchAction::Delete { name: "feat/y".into(), force: true }).await.unwrap();
    b(BranchAction::Delete { name: "dev2".into(), force: false }).await.unwrap();
    // validación
    for bad in ["-x", "a..b", "x@{1}", "foo.lock", "a//b", "--upload-pack=evil", "HEAD:evil"] {
        assert!(b(BranchAction::Create { name: bad.into(), start: None, checkout: false }).await.is_err(), "{bad}");
    }
    assert!(b(BranchAction::Create { name: "ok".into(), start: Some("-x".into()), checkout: false }).await.is_err());
    // detach
    let sha = r.g(&["rev-parse", "HEAD"]);
    ops::checkout_commit(&r.state, None, sha).await.unwrap();
    assert!(status2::repo_info(&r.state, None).await.unwrap().detached);
}

#[tokio::test]
async fn merge_conflict_resolution_flow() {
    let Some(r) = TestRepo::new() else { return };
    r.commit_file("f.txt", "base\n", "base");
    r.g(&["switch", "-q", "-c", "other"]);
    r.commit_file("f.txt", "theirs\n", "other change");
    r.g(&["switch", "-q", "main"]);
    r.commit_file("f.txt", "ours\n", "main change");

    let o = ops::merge(&r.state, None, "other".into(), false, false).await.unwrap();
    assert!(!o.ok);
    assert_eq!(o.conflicts, vec!["f.txt".to_string()]);
    let st = status2::status_v2(&r.state, None).await.unwrap();
    assert_eq!(st.info.state, RepoState::Merging);
    assert_eq!(st.conflicted.len(), 1);
    assert_eq!(st.conflicted[0].conflict.as_deref(), Some("UU"));

    ops::conflict(&r.state, None, "f.txt".into(), ConflictAction::Ours).await.unwrap();
    assert_eq!(std::fs::read_to_string(r.root.join("f.txt")).unwrap(), "ours\n");
    let o = ops::sequence(&r.state, None, SeqOp::Merge, SeqAction::Continue).await.unwrap();
    assert!(o.ok);
    assert_eq!(status2::repo_info(&r.state, None).await.unwrap().state, RepoState::Clean);
    assert_eq!(log::commit_detail(&r.state, None, "HEAD".into()).await.unwrap().row.parents.len(), 2);

    // abort
    r.g(&["reset", "-q", "--hard", "HEAD~1"]);
    let o = ops::merge(&r.state, None, "other".into(), false, false).await.unwrap();
    assert!(!o.ok);
    ops::sequence(&r.state, None, SeqOp::Merge, SeqAction::Abort).await.unwrap();
    assert_eq!(status2::repo_info(&r.state, None).await.unwrap().state, RepoState::Clean);
    assert!(ops::sequence(&r.state, None, SeqOp::Merge, SeqAction::Skip).await.is_err());

    // theirs + markResolved
    let o = ops::merge(&r.state, None, "other".into(), false, false).await.unwrap();
    assert!(!o.ok);
    ops::conflict(&r.state, None, "f.txt".into(), ConflictAction::Theirs).await.unwrap();
    assert_eq!(std::fs::read_to_string(r.root.join("f.txt")).unwrap(), "theirs\n");
    ops::sequence(&r.state, None, SeqOp::Merge, SeqAction::Continue).await.unwrap();
    let _ = ConflictAction::MarkResolved;
    // merge normal (no-op: ya fusionado)
    assert!(ops::merge(&r.state, None, "other".into(), true, true).await.is_err());
}

#[tokio::test]
async fn rebase_cherry_pick_revert_reset() {
    let Some(r) = TestRepo::new() else { return };
    r.commit_file("a.txt", "a\n", "c1");
    r.g(&["switch", "-q", "-c", "topic"]);
    let t1 = r.commit_file("t.txt", "t\n", "topic 1");
    r.g(&["switch", "-q", "main"]);
    r.commit_file("m.txt", "m\n", "main 2");

    // cherry-pick
    let o = ops::cherry_pick(&r.state, None, vec![t1.clone()]).await.unwrap();
    assert!(o.ok);
    let d = log::commit_detail(&r.state, None, "HEAD".into()).await.unwrap();
    assert_eq!(d.row.subject, "topic 1");
    assert!(d.body.contains("cherry picked from commit"));
    // revert
    let head = r.g(&["rev-parse", "HEAD"]);
    ops::revert(&r.state, None, head).await.unwrap();
    assert!(!r.root.join("t.txt").exists());
    assert!(ops::cherry_pick(&r.state, None, vec![]).await.is_err());
    assert!(ops::cherry_pick(&r.state, None, vec!["-x".into()]).await.is_err());

    // rebase de topic sobre main
    r.g(&["switch", "-q", "topic"]);
    r.commit_file("t2.txt", "t2\n", "topic 2");
    let o = ops::rebase(&r.state, None, "main".into()).await.unwrap();
    assert!(o.ok);
    let parents = log::commit_detail(&r.state, None, "HEAD".into()).await.unwrap().row.parents;
    assert_eq!(parents[0], r.g(&["rev-parse", "main"]));
    assert!(ops::rebase(&r.state, None, "--exec=evil".into()).await.is_err());

    // reset
    let base = r.g(&["rev-parse", "HEAD~1"]);
    ops::reset(&r.state, None, base.clone(), ResetMode::Soft, false).await.unwrap();
    assert_eq!(status2::status_v2(&r.state, None).await.unwrap().staged.len(), 1);
    ops::reset(&r.state, None, base.clone(), ResetMode::Mixed, false).await.unwrap();
    assert_eq!(status2::status_v2(&r.state, None).await.unwrap().untracked.len(), 1);
    assert!(ops::reset(&r.state, None, base.clone(), ResetMode::Hard, false).await.is_err());
    assert!(r.root.join("t2.txt").exists());
    r.g(&["add", "t2.txt"]);
    ops::reset(&r.state, None, base, ResetMode::Hard, true).await.unwrap();
    assert!(!r.root.join("t2.txt").exists());
}

#[tokio::test]
async fn rebase_conflict_continue() {
    let Some(r) = TestRepo::new() else { return };
    r.commit_file("f.txt", "base\n", "base");
    r.g(&["switch", "-q", "-c", "topic"]);
    r.commit_file("f.txt", "topic\n", "topic");
    r.g(&["switch", "-q", "main"]);
    r.commit_file("f.txt", "main\n", "main");
    r.g(&["switch", "-q", "topic"]);
    let o = ops::rebase(&r.state, None, "main".into()).await.unwrap();
    assert!(!o.ok);
    assert_eq!(status2::repo_info(&r.state, None).await.unwrap().state, RepoState::Rebasing);
    r.write("f.txt", "resuelto\n");
    ops::conflict(&r.state, None, "f.txt".into(), ConflictAction::MarkResolved).await.unwrap();
    let o = ops::sequence(&r.state, None, SeqOp::Rebase, SeqAction::Continue).await.unwrap();
    assert!(o.ok);
    assert_eq!(status2::repo_info(&r.state, None).await.unwrap().state, RepoState::Clean);
}

#[tokio::test]
async fn stash_and_tags() {
    let Some(r) = TestRepo::new() else { return };
    r.commit_file("a.txt", "a\n", "c1");
    r.write("a.txt", "changed\n");
    r.write("u.txt", "untracked\n");
    ops::stash(&r.state, None, StashAction::Push { message: Some("--raro".into()), include_untracked: true, keep_index: false }).await.unwrap();
    assert!(!r.root.join("u.txt").exists());
    let rf = refs::refs(&r.state, None).await.unwrap();
    assert_eq!(rf.stashes.len(), 1);
    assert!(rf.stashes[0].message.contains("--raro"));
    ops::stash(&r.state, None, StashAction::Apply { index: 0 }).await.unwrap();
    assert!(r.root.join("u.txt").exists());
    r.g(&["checkout", "--", "a.txt"]);
    std::fs::remove_file(r.root.join("u.txt")).unwrap();
    ops::stash(&r.state, None, StashAction::Pop { index: 0 }).await.unwrap();
    assert!(r.root.join("u.txt").exists());
    assert!(refs::refs(&r.state, None).await.unwrap().stashes.is_empty());
    r.write("a.txt", "again\n");
    ops::stash(&r.state, None, StashAction::Push { message: None, include_untracked: false, keep_index: false }).await.unwrap();
    ops::stash(&r.state, None, StashAction::Drop { index: 0 }).await.unwrap();
    assert!(refs::refs(&r.state, None).await.unwrap().stashes.is_empty());

    let sha = r.g(&["rev-parse", "HEAD"]);
    ops::tag(&r.state, None, TagAction::Create { name: "v1".into(), sha: sha.clone(), message: None }).await.unwrap();
    ops::tag(&r.state, None, TagAction::Create { name: "v2".into(), sha: sha.clone(), message: Some("Release \"2\"\n\ncuerpo".into()) }).await.unwrap();
    let t = refs::refs(&r.state, None).await.unwrap().tags;
    assert_eq!(t.len(), 2);
    assert!(t.iter().find(|x| x.name == "v2").unwrap().annotated);
    ops::tag(&r.state, None, TagAction::Delete { name: "v1".into() }).await.unwrap();
    assert_eq!(refs::refs(&r.state, None).await.unwrap().tags.len(), 1);
    assert!(ops::tag(&r.state, None, TagAction::Create { name: "-bad".into(), sha, message: None }).await.is_err());
    assert!(ops::tag(&r.state, None, TagAction::Push { remote: "noexiste".into(), name: "v2".into() }).await.is_err());
}

#[tokio::test]
async fn discard_untracked_goes_to_trash_api() {
    let Some(r) = TestRepo::new() else { return };
    r.commit_file("a.txt", "a\n", "c1");
    // Archivo rastreado marcado como untracked: rechazado (no toca la papelera del sistema).
    assert!(ops::discard(&r.state, None, s(&["a.txt"]), true).await.is_err());
    assert!(r.root.join("a.txt").exists());
}

#[tokio::test]
async fn remote_fetch_pull_push_with_progress() {
    let Some(r) = TestRepo::new() else { return };
    let bare = tempfile::tempdir().unwrap();
    git_sync(bare.path(), &["init", "-q", "--bare", "-b", "main"]);
    let bare_path = bare.path().to_string_lossy().into_owned();
    r.commit_file("a.txt", "1\n", "c1");
    r.g(&["remote", "add", "origin", &bare_path]);

    // push con set_upstream sin indicar remoto ni rama
    let mut events: Vec<GitProgress> = Vec::new();
    let o = remote::run_remote(
        &r.state, None,
        RemoteOp::Push { remote: None, branch: None, force_with_lease: false, set_upstream: true, tags: false },
        "job1".into(),
        |ev| {
            events.push(ev);
            true
        },
    )
    .await
    .unwrap();
    assert!(o.ok);
    assert_eq!(events.last(), Some(&GitProgress::Done));
    assert_eq!(status2::repo_info(&r.state, None).await.unwrap().upstream.as_deref(), Some("origin/main"));
    assert!(r.state.git_jobs.lock().unwrap().is_empty());

    // segundo clon empuja un commit; fetch + pull lo traen
    let other = tempfile::tempdir().unwrap();
    git_sync(other.path(), &["clone", "-q", &bare_path, "."]);
    git_sync(other.path(), &["config", "user.name", "O"]);
    git_sync(other.path(), &["config", "user.email", "o@x.io"]);
    git_sync(other.path(), &["config", "commit.gpgsign", "false"]);
    std::fs::write(other.path().join("o.txt"), "o\n").unwrap();
    git_sync(other.path(), &["add", "o.txt"]);
    git_sync(other.path(), &["commit", "-q", "-m", "from other"]);
    git_sync(other.path(), &["push", "-q"]);

    let mut ev2 = Vec::new();
    remote::run_remote(&r.state, None, RemoteOp::Fetch { remote: Some("origin".into()), prune: true }, "job2".into(), |e| {
        ev2.push(e);
        true
    })
    .await
    .unwrap();
    assert_eq!(ev2.last(), Some(&GitProgress::Done));
    assert_eq!(status2::repo_info(&r.state, None).await.unwrap().behind, 1);

    remote::run_remote(&r.state, None, RemoteOp::Pull { rebase: false, ff_only: false }, "job3".into(), |_| true).await.unwrap();
    assert!(r.root.join("o.txt").exists());

    // push normal tras un commit local
    r.commit_file("l.txt", "l\n", "local");
    remote::run_remote(
        &r.state, None,
        RemoteOp::Push { remote: Some("origin".into()), branch: Some("main".into()), force_with_lease: false, set_upstream: false, tags: false },
        "job4".into(),
        |_| true,
    )
    .await
    .unwrap();
    assert_eq!(git_sync(bare.path(), &["rev-parse", "main"]), r.g(&["rev-parse", "HEAD"]));

    // push de un tag
    let sha = r.g(&["rev-parse", "HEAD"]);
    ops::tag(&r.state, None, TagAction::Create { name: "rel".into(), sha, message: None }).await.unwrap();
    ops::tag(&r.state, None, TagAction::Push { remote: "origin".into(), name: "rel".into() }).await.unwrap();
    assert!(git_sync(bare.path(), &["tag"]).contains("rel"));

    // remoto inexistente / rama con guion: validación, y fallo emite Failed
    let mut ev3 = Vec::new();
    let e = remote::run_remote(&r.state, None, RemoteOp::Fetch { remote: Some("nope".into()), prune: false }, "job5".into(), |e| {
        ev3.push(e);
        true
    })
    .await;
    assert!(e.is_err());
    assert!(matches!(ev3.last(), Some(GitProgress::Failed { .. })));
    assert!(remote::run_remote(
        &r.state, None,
        RemoteOp::Push { remote: Some("origin".into()), branch: Some("--delete".into()), force_with_lease: false, set_upstream: false, tags: false },
        "job6".into(),
        |_| true
    )
    .await
    .is_err());
    // pull --ff-only con historias divergentes falla con error (no conflicto)
    r.commit_file("div.txt", "d\n", "local div");
    git_sync(other.path(), &["pull", "-q", "--rebase"]);
    std::fs::write(other.path().join("o2.txt"), "o2\n").unwrap();
    git_sync(other.path(), &["add", "o2.txt"]);
    git_sync(other.path(), &["commit", "-q", "-m", "other 2"]);
    git_sync(other.path(), &["push", "-q"]);
    assert!(remote::run_remote(&r.state, None, RemoteOp::Pull { rebase: false, ff_only: false }, "job7".into(), |_| true).await.is_err());
    assert_eq!(r.state.git_busy_total(), 0);
}

#[tokio::test]
async fn remote_cancel_aborts_job() {
    let Some(r) = TestRepo::new() else { return };
    r.commit_file("a.txt", "1\n", "c1");
    // Remoto inexistente que cuelga: un helper "ext::" lento no es portable; se usa un remoto local y
    // se cancela antes de empezar (el cancel llega mientras el job está en cola de `git_op`).
    r.g(&["remote", "add", "origin", &r.root.to_string_lossy()]);
    let guard = super::exec::OpGuard::acquire(&r.state, &r.root).await;
    let st = &r.state;
    let fut = remote::run_remote(st, None, RemoteOp::Fetch { remote: None, prune: false }, "jobc".into(), |_| true);
    tokio::pin!(fut);
    // Dejar que el job se registre y se bloquee en el lock.
    tokio::select! {
        _ = &mut fut => panic!("no debía terminar con el lock tomado"),
        _ = tokio::time::sleep(std::time::Duration::from_millis(300)) => {}
    }
    assert!(remote::cancel(st, "jobc"));
    drop(guard);
    let e = fut.await.unwrap_err();
    assert!(e.to_string().contains("cancelada"));
    assert!(!remote::cancel(st, "jobc"));
}

#[tokio::test]
async fn blame_and_file_history_with_rename() {
    let Some(r) = TestRepo::new() else { return };
    r.commit_file("old.txt", "uno\ndos\ntres\n", "crear");
    r.g(&["mv", "old.txt", "nuevo.txt"]);
    r.g(&["commit", "-q", "-m", "renombrar"]);
    r.write("nuevo.txt", "uno\nDOS\ntres\n");
    r.g(&["commit", "-q", "-a", "-m", "editar dos"]);
    let b = blame::blame(&r.state, None, "nuevo.txt".into(), None).await.unwrap();
    assert_eq!(b.lines.len(), 3);
    assert_eq!(b.commits.len(), 2);
    let sha2 = &b.lines[1].1;
    assert_eq!(b.commits[sha2].summary, "editar dos");
    assert_eq!(b.commits[sha2].author, "Test");
    let h = log::file_history(&r.state, None, "nuevo.txt".into(), 0, 10).await.unwrap();
    let subjects: Vec<_> = h.commits.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, vec!["editar dos", "renombrar", "crear"]);
    assert!(blame::blame(&r.state, None, "nuevo.txt".into(), Some("-x".into())).await.is_err());
}

#[tokio::test]
async fn workspace_subfolder_of_repo_works() {
    // El workspace puede ser una subcarpeta del repo: los pathspec son relativos al top.
    let Some(r) = TestRepo::new() else { return };
    r.commit_file("a.txt", "a\n", "c1");
    r.write("pkg/app/x.txt", "x\n");
    let sub = r.root.join("pkg/app");
    r.state.workspace.write().unwrap().replace(crate::workspace::Workspace::single(sub.clone()));
    ops::stage(&r.state, None, vec![sub.join("x.txt").to_string_lossy().into_owned()]).await.unwrap();
    let st = status2::status_v2(&r.state, None).await.unwrap();
    assert_eq!(st.staged.len(), 1);
    assert_eq!(st.staged[0].path, "pkg/app/x.txt");
    // fuera del workspace (aunque dentro del repo): rechazado
    assert!(ops::stage(&r.state, None, vec![r.root.join("a.txt").to_string_lossy().into_owned()]).await.is_err());
}
