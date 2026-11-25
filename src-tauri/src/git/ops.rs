//! Operaciones mutadoras (stage, commit, ramas, merge, rebase, stash, tags...).
//! Cada función pública toma `OpGuard` (serializa con `git_op` y marca `git_busy`); los helpers
//! internos NO lo toman (tokio::Mutex no es reentrante).

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::exec::{ctx_for, run_raw, run_str, sanitize_text, Ctx, OpGuard, RawOut, RunOpts, MUT_TIMEOUT};
use super::validate;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub const MAX_PATCH_BYTES: usize = 5 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpOutcome {
    pub ok: bool,
    pub conflicts: Vec<String>,
    pub message: String,
}

fn trim_msg(s: &str) -> String {
    let s = sanitize_text(s.trim());
    if s.chars().count() > 4000 {
        let cut: String = s.chars().take(4000).collect();
        format!("{cut}…")
    } else {
        s
    }
}

fn combined(out: &RawOut) -> String {
    let mut m = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let e = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !e.is_empty() {
        if !m.is_empty() {
            m.push('\n');
        }
        m.push_str(&e);
    }
    trim_msg(&m)
}

pub async fn list_conflicts(state: &AppState, top: &Path) -> Vec<String> {
    match run_raw(state, top, &["diff", "--name-only", "--diff-filter=U", "-z"], RunOpts::default()).await {
        Ok(o) if o.ok() => String::from_utf8_lossy(&o.stdout).split('\0').filter(|s| !s.is_empty()).map(str::to_string).collect(),
        _ => vec![],
    }
}

/// Resultado de una operación que puede dejar conflictos: éxito → `ok`; fallo con conflictos →
/// `Ok(ok:false, conflicts)`; cualquier otro fallo → `Err`.
async fn finish(state: &AppState, c: &Ctx, out: RawOut) -> AppResult<OpOutcome> {
    if out.ok() {
        return Ok(OpOutcome { ok: true, conflicts: vec![], message: combined(&out) });
    }
    let conflicts = list_conflicts(state, &c.top).await;
    if conflicts.is_empty() {
        return Err(AppError::Git(trim_msg(&out.message())));
    }
    Ok(OpOutcome { ok: false, conflicts, message: combined(&out) })
}

/// Fallo → `Err`; éxito → `OpOutcome`. Para ops que no producen conflictos.
fn plain(out: RawOut) -> AppResult<OpOutcome> {
    if out.ok() {
        Ok(OpOutcome { ok: true, conflicts: vec![], message: combined(&out) })
    } else {
        Err(AppError::Git(trim_msg(&out.message())))
    }
}

async fn run_mut<S: AsRef<str>>(state: &AppState, c: &Ctx, args: &[S], opts: RunOpts) -> AppResult<RawOut> {
    run_raw(state, &c.top, args, opts.timeout(MUT_TIMEOUT)).await
}

async fn has_head(state: &AppState, c: &Ctx) -> bool {
    run_raw(state, &c.top, &["rev-parse", "--verify", "-q", "HEAD"], RunOpts::default()).await.map(|o| o.ok()).unwrap_or(false)
}

fn dashdash(mut args: Vec<String>, paths: Vec<String>) -> Vec<String> {
    args.push("--".into());
    args.extend(paths);
    args
}

// ---------------------------------------------------------------------------
// Stage / unstage / discard / apply
// ---------------------------------------------------------------------------

pub async fn stage(state: &AppState, repo: Option<&str>, paths: Vec<String>) -> AppResult<OpOutcome> {
    let c = ctx_for(state, repo).await?;
    let rel = validate::rel_paths(&c, &paths)?;
    let _g = OpGuard::acquire(state, &c.top).await;
    plain(run_mut(state, &c, &dashdash(vec!["add".into(), "-A".into()], rel), RunOpts::default()).await?)
}

pub async fn unstage(state: &AppState, repo: Option<&str>, paths: Vec<String>) -> AppResult<OpOutcome> {
    let c = ctx_for(state, repo).await?;
    let rel = validate::rel_paths(&c, &paths)?;
    let _g = OpGuard::acquire(state, &c.top).await;
    let args = if has_head(state, &c).await {
        dashdash(vec!["restore".into(), "--staged".into()], rel)
    } else {
        dashdash(vec!["rm".into(), "--cached".into(), "-r".into(), "-q".into()], rel)
    };
    plain(run_mut(state, &c, &args, RunOpts::default()).await?)
}

/// Descarta cambios del worktree. `untracked`: los archivos van a la papelera (nunca `clean -f`).
pub async fn discard(state: &AppState, repo: Option<&str>, paths: Vec<String>, untracked: bool) -> AppResult<OpOutcome> {
    let c = ctx_for(state, repo).await?;
    let rel = validate::rel_paths(&c, &paths)?;
    let _g = OpGuard::acquire(state, &c.top).await;
    if untracked {
        let mut n = 0;
        for r in &rel {
            let abs = c.top.join(r);
            // Solo lo que realmente es untracked: un archivo rastreado no se manda a la papelera.
            let o = run_raw(state, &c.top, &["ls-files", "-z", "--others", "--exclude-standard", "--", r.as_str()], RunOpts::default()).await?;
            if !o.ok() || o.stdout.is_empty() {
                return Err(AppError::InvalidArgument(format!("no es un archivo sin seguimiento: {r}")));
            }
            crate::fs::ops::delete(&abs)?;
            n += 1;
        }
        return Ok(OpOutcome { ok: true, conflicts: vec![], message: format!("{n} a la papelera") });
    }
    plain(run_mut(state, &c, &dashdash(vec!["restore".into(), "--worktree".into()], rel), RunOpts::default()).await?)
}

/// Stage / unstage / discard por hunk. `cached`+`reverse` = unstage; `!cached`+`reverse` = descartar hunk.
pub async fn apply_patch(state: &AppState, repo: Option<&str>, patch: String, cached: bool, reverse: bool) -> AppResult<OpOutcome> {
    if patch.trim().is_empty() {
        return Err(AppError::InvalidArgument("parche vacío".into()));
    }
    if patch.len() > MAX_PATCH_BYTES {
        return Err(AppError::InvalidArgument("parche demasiado grande".into()));
    }
    let c = ctx_for(state, repo).await?;
    let mut args: Vec<&str> = vec!["apply"];
    if cached {
        args.push("--cached");
    }
    if reverse {
        args.push("-R");
    }
    args.extend(["--recount", "--whitespace=nowarn", "-"]);
    let _g = OpGuard::acquire(state, &c.top).await;
    plain(run_mut(state, &c, &args, RunOpts::stdin(patch.into_bytes())).await?)
}

// ---------------------------------------------------------------------------
// Commit
// ---------------------------------------------------------------------------

pub async fn commit(state: &AppState, repo: Option<&str>, message: String, amend: bool, sign_off: bool) -> AppResult<OpOutcome> {
    let c = ctx_for(state, repo).await?;
    let empty = message.trim().is_empty();
    if empty && !amend {
        return Err(AppError::InvalidArgument("el mensaje de commit está vacío".into()));
    }
    if message.contains('\0') {
        return Err(AppError::InvalidArgument("mensaje con NUL".into()));
    }
    let mut args: Vec<&str> = vec!["commit"];
    let opts = if empty {
        args.push("--no-edit");
        RunOpts::default()
    } else {
        args.extend(["-F", "-"]);
        RunOpts::stdin(message.into_bytes())
    };
    if amend {
        args.push("--amend");
    }
    if sign_off {
        args.push("-s");
    }
    let _g = OpGuard::acquire(state, &c.top).await;
    plain(run_mut(state, &c, &args, opts.no_editor()).await?)
}

// ---------------------------------------------------------------------------
// Ramas
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum BranchAction {
    Create { name: String, start: Option<String>, checkout: bool },
    Delete { name: String, force: bool },
    Rename { from: String, to: String },
    Checkout { name: String },
    CheckoutRemote { remote_branch: String },
    SetUpstream { name: String, upstream: String },
}

pub async fn branch(state: &AppState, repo: Option<&str>, action: BranchAction) -> AppResult<OpOutcome> {
    let c = ctx_for(state, repo).await?;
    let args: Vec<String> = match action {
        BranchAction::Create { name, start, checkout } => {
            validate::check_branch_name(state, &c, &name).await?;
            if let Some(s) = &start {
                validate::rev("punto de partida", s)?;
            }
            let mut a: Vec<String> = if checkout { vec!["switch".into(), "-c".into(), name] } else { vec!["branch".into(), name] };
            a.extend(start);
            a
        }
        BranchAction::Delete { name, force } => {
            validate::refname("rama", &name)?;
            vec!["branch".into(), if force { "-D" } else { "-d" }.into(), name]
        }
        BranchAction::Rename { from, to } => {
            validate::refname("rama", &from)?;
            validate::check_branch_name(state, &c, &to).await?;
            vec!["branch".into(), "-m".into(), from, to]
        }
        BranchAction::Checkout { name } => {
            validate::refname("rama", &name)?;
            vec!["switch".into(), name]
        }
        BranchAction::CheckoutRemote { remote_branch } => {
            validate::refname("rama remota", &remote_branch)?;
            let (remote, _) = remote_branch
                .split_once('/')
                .ok_or_else(|| AppError::InvalidArgument("se esperaba remoto/rama".into()))?;
            validate::remote(state, &c, remote).await?;
            vec!["switch".into(), "--track".into(), remote_branch]
        }
        BranchAction::SetUpstream { name, upstream } => {
            validate::refname("rama", &name)?;
            validate::refname("upstream", &upstream)?;
            vec!["branch".into(), format!("--set-upstream-to={upstream}"), name]
        }
    };
    let _g = OpGuard::acquire(state, &c.top).await;
    plain(run_mut(state, &c, &args, RunOpts::default()).await?)
}

pub async fn checkout_commit(state: &AppState, repo: Option<&str>, sha: String) -> AppResult<OpOutcome> {
    validate::rev("sha", &sha)?;
    let c = ctx_for(state, repo).await?;
    let _g = OpGuard::acquire(state, &c.top).await;
    plain(run_mut(state, &c, &["switch", "--detach", sha.as_str()], RunOpts::default()).await?)
}

// ---------------------------------------------------------------------------
// merge / rebase / cherry-pick / revert / reset / sequence / conflict
// ---------------------------------------------------------------------------

pub async fn merge(state: &AppState, repo: Option<&str>, name: String, no_ff: bool, squash: bool) -> AppResult<OpOutcome> {
    validate::rev("rama", &name)?;
    if no_ff && squash {
        return Err(AppError::InvalidArgument("no_ff y squash son excluyentes".into()));
    }
    let c = ctx_for(state, repo).await?;
    let mut args: Vec<&str> = vec!["merge", "--no-edit"];
    if no_ff {
        args.push("--no-ff");
    }
    if squash {
        args.push("--squash");
    }
    args.push(&name);
    let _g = OpGuard::acquire(state, &c.top).await;
    let out = run_mut(state, &c, &args, RunOpts::default().no_editor()).await?;
    finish(state, &c, out).await
}

pub async fn rebase(state: &AppState, repo: Option<&str>, onto: String) -> AppResult<OpOutcome> {
    validate::rev("onto", &onto)?;
    let c = ctx_for(state, repo).await?;
    let _g = OpGuard::acquire(state, &c.top).await;
    let out = run_mut(state, &c, &["rebase", onto.as_str()], RunOpts::default().no_editor()).await?;
    finish(state, &c, out).await
}

pub async fn cherry_pick(state: &AppState, repo: Option<&str>, shas: Vec<String>) -> AppResult<OpOutcome> {
    if shas.is_empty() {
        return Err(AppError::InvalidArgument("sin commits".into()));
    }
    for s in &shas {
        validate::sha(s)?;
    }
    let c = ctx_for(state, repo).await?;
    let mut args: Vec<String> = vec!["cherry-pick".into(), "-x".into()];
    args.extend(shas);
    let _g = OpGuard::acquire(state, &c.top).await;
    let out = run_mut(state, &c, &args, RunOpts::default().no_editor()).await?;
    finish(state, &c, out).await
}

async fn is_merge_commit(state: &AppState, c: &Ctx, sha: &str) -> bool {
    match run_str(state, &c.top, &["rev-list", "--parents", "-n", "1", sha]).await {
        Ok(s) => s.split_whitespace().count() > 2,
        Err(_) => false,
    }
}

pub async fn revert(state: &AppState, repo: Option<&str>, sha: String) -> AppResult<OpOutcome> {
    validate::sha(&sha)?;
    let c = ctx_for(state, repo).await?;
    let _g = OpGuard::acquire(state, &c.top).await;
    let mut args: Vec<&str> = vec!["revert", "--no-edit"];
    if is_merge_commit(state, &c, &sha).await {
        args.extend(["-m", "1"]);
    }
    args.push(&sha);
    let out = run_mut(state, &c, &args, RunOpts::default().no_editor()).await?;
    finish(state, &c, out).await
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

pub async fn reset(state: &AppState, repo: Option<&str>, sha: String, mode: ResetMode, confirm: bool) -> AppResult<OpOutcome> {
    validate::rev("sha", &sha)?;
    if mode == ResetMode::Hard && !confirm {
        return Err(AppError::InvalidArgument("reset --hard requiere confirm: true".into()));
    }
    let flag = match mode {
        ResetMode::Soft => "--soft",
        ResetMode::Mixed => "--mixed",
        ResetMode::Hard => "--hard",
    };
    let c = ctx_for(state, repo).await?;
    let _g = OpGuard::acquire(state, &c.top).await;
    plain(run_mut(state, &c, &["reset", flag, sha.as_str()], RunOpts::default()).await?)
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SeqOp {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SeqAction {
    Abort,
    Continue,
    Skip,
}

pub async fn sequence(state: &AppState, repo: Option<&str>, op: SeqOp, action: SeqAction) -> AppResult<OpOutcome> {
    let cmd = match op {
        SeqOp::Merge => "merge",
        SeqOp::Rebase => "rebase",
        SeqOp::CherryPick => "cherry-pick",
        SeqOp::Revert => "revert",
    };
    let flag = match action {
        SeqAction::Abort => "--abort",
        SeqAction::Continue => "--continue",
        SeqAction::Skip => {
            if op == SeqOp::Merge {
                return Err(AppError::InvalidArgument("merge no admite --skip".into()));
            }
            "--skip"
        }
    };
    let c = ctx_for(state, repo).await?;
    let _g = OpGuard::acquire(state, &c.top).await;
    let out = run_mut(state, &c, &[cmd, flag], RunOpts::default().no_editor()).await?;
    finish(state, &c, out).await
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConflictAction {
    Ours,
    Theirs,
    MarkResolved,
}

pub async fn conflict(state: &AppState, repo: Option<&str>, path: String, action: ConflictAction) -> AppResult<OpOutcome> {
    let c = ctx_for(state, repo).await?;
    let rel = validate::rel_path(&c, &path)?;
    let _g = OpGuard::acquire(state, &c.top).await;
    let side = match action {
        ConflictAction::Ours => Some("--ours"),
        ConflictAction::Theirs => Some("--theirs"),
        ConflictAction::MarkResolved => None,
    };
    if let Some(side) = side {
        let r = run_mut(state, &c, &["checkout", side, "--", rel.as_str()], RunOpts::default()).await?;
        if !r.ok() {
            return Err(AppError::Git(trim_msg(&r.message())));
        }
    }
    plain(run_mut(state, &c, &["add", "--", rel.as_str()], RunOpts::default()).await?)
}

// ---------------------------------------------------------------------------
// Stash
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StashAction {
    Push { message: Option<String>, include_untracked: bool, keep_index: bool },
    Pop { index: u32 },
    Apply { index: u32 },
    Drop { index: u32 },
}

pub async fn stash(state: &AppState, repo: Option<&str>, action: StashAction) -> AppResult<OpOutcome> {
    let c = ctx_for(state, repo).await?;
    let (args, conflicts_possible): (Vec<String>, bool) = match action {
        StashAction::Push { message, include_untracked, keep_index } => {
            let mut a: Vec<String> = vec!["stash".into(), "push".into()];
            if include_untracked {
                a.push("-u".into());
            }
            if keep_index {
                a.push("--keep-index".into());
            }
            if let Some(m) = message.filter(|m| !m.trim().is_empty()) {
                if m.contains('\0') {
                    return Err(AppError::InvalidArgument("mensaje con NUL".into()));
                }
                a.push(format!("--message={m}"));
            }
            (a, false)
        }
        StashAction::Pop { index } => (vec!["stash".into(), "pop".into(), format!("stash@{{{index}}}")], true),
        StashAction::Apply { index } => (vec!["stash".into(), "apply".into(), format!("stash@{{{index}}}")], true),
        StashAction::Drop { index } => (vec!["stash".into(), "drop".into(), format!("stash@{{{index}}}")], false),
    };
    let _g = OpGuard::acquire(state, &c.top).await;
    let out = run_mut(state, &c, &args, RunOpts::default()).await?;
    if conflicts_possible {
        finish(state, &c, out).await
    } else {
        plain(out)
    }
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TagAction {
    Create { name: String, sha: String, message: Option<String> },
    Delete { name: String },
    Push { remote: String, name: String },
}

pub async fn tag(state: &AppState, repo: Option<&str>, action: TagAction) -> AppResult<OpOutcome> {
    let c = ctx_for(state, repo).await?;
    match action {
        TagAction::Create { name, sha, message } => {
            validate::check_tag_name(state, &c, &name).await?;
            validate::rev("sha", &sha)?;
            let msg = message.filter(|m| !m.trim().is_empty());
            let (args, opts): (Vec<&str>, RunOpts) = match &msg {
                Some(m) => (vec!["tag", "-a", "-F", "-", name.as_str(), sha.as_str()], RunOpts::stdin(m.clone().into_bytes())),
                None => (vec!["tag", name.as_str(), sha.as_str()], RunOpts::default()),
            };
            let _g = OpGuard::acquire(state, &c.top).await;
            plain(run_mut(state, &c, &args, opts).await?)
        }
        TagAction::Delete { name } => {
            validate::refname("tag", &name)?;
            let _g = OpGuard::acquire(state, &c.top).await;
            plain(run_mut(state, &c, &["tag", "-d", name.as_str()], RunOpts::default()).await?)
        }
        TagAction::Push { remote, name } => {
            validate::remote(state, &c, &remote).await?;
            validate::refname("tag", &name)?;
            let refspec = format!("refs/tags/{name}");
            let _g = OpGuard::acquire(state, &c.top).await;
            let opts = super::remote::remote_env().into_iter().fold(RunOpts::default(), |o, (k, v)| o.env(&k, &v));
            let out = run_raw(
                state,
                &c.top,
                &["-c", "http.lowSpeedLimit=1000", "-c", "http.lowSpeedTime=30", "push", remote.as_str(), refspec.as_str()],
                opts.timeout(super::exec::REMOTE_TIMEOUT),
            )
            .await?;
            plain(out)
        }
    }
}

// ---------------------------------------------------------------------------
// init
// ---------------------------------------------------------------------------

/// `git init` en una carpeta de alguna raíz del workspace sin repo; rama inicial `main`.
pub async fn init(state: &AppState, path: String) -> AppResult<OpOutcome> {
    let roots = state.roots()?;
    let (p, _) = crate::util::paths::ensure_in_roots(&roots, Path::new(&path))?;
    if !p.is_dir() {
        return Err(AppError::InvalidArgument("la ruta no es una carpeta".into()));
    }
    let _g = OpGuard::acquire(state, &p).await;
    let inside = run_raw(state, &p, &["rev-parse", "--git-dir"], RunOpts::default()).await?;
    if inside.ok() {
        return Err(AppError::AlreadyExists("ya es un repositorio git".into()));
    }
    let out = run_raw(state, &p, &["init"], RunOpts::default()).await?;
    if !out.ok() {
        return Err(AppError::Git(trim_msg(&out.message())));
    }
    let _ = run_raw(state, &p, &["symbolic-ref", "HEAD", "refs/heads/main"], RunOpts::default()).await;
    // La detección cacheada ya no es válida: aparece un repo nuevo.
    if let Ok(mut g) = state.git_repos.lock() {
        g.clear();
    }
    Ok(OpOutcome { ok: true, conflicts: vec![], message: combined(&out) })
}
