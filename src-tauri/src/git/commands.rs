use tauri::ipc::Channel;
use tauri::State;

use super::blame::{self, BlameResult};
use super::diff::{self, DiffTarget, FileDiff};
use super::log::{self, CommitDetail, LogPage, LogQuery};
use super::ops::{self, BranchAction, ConflictAction, OpOutcome, ResetMode, SeqAction, SeqOp, StashAction, TagAction};
use super::refs::{self, Refs};
use super::remote::{self, GitProgress, RemoteOp};
use super::status2::{self, RepoInfo, StatusV2};
use super::exec::ctx_for;
use super::repos::{self, RepoSummary, ReposResult};
use super::{parse_porcelain, run_git, GitStatus};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub async fn git_status(state: State<'_, AppState>, repo: Option<String>) -> AppResult<GitStatus> {
    // Sin git en PATH, fuera de un repo o en un repo ilegible: no es un error, simplemente no hay repo.
    let Ok(c) = ctx_for(&state, repo.as_deref()).await else {
        return Ok(GitStatus::not_repo());
    };
    // Las rutas de porcelain son relativas al toplevel del repo (puede ser un ancestro de la raíz).
    let base = c.top.clone();

    let (status, branch) = tokio::join!(
        run_git(&state, &c.top, &["status", "--porcelain=v1", "-z", "--untracked-files=all"]),
        current_branch(&state, &c.top),
    );
    let Ok(raw) = status else {
        return Ok(GitStatus::not_repo());
    };
    Ok(GitStatus {
        is_repo: true,
        branch,
        files: parse_porcelain(&base, &raw),
    })
}

async fn current_branch(state: &AppState, root: &std::path::Path) -> Option<String> {
    if let Ok(out) = run_git(state, root, &["rev-parse", "--abbrev-ref", "HEAD"]).await {
        let b = out.trim();
        if b == "HEAD" {
            return Some("detached".into());
        }
        if !b.is_empty() {
            return Some(b.to_string());
        }
    }
    // Repo recién creado sin commits: HEAD no resuelve, pero la rama simbólica sí.
    run_git(state, root, &["symbolic-ref", "--short", "-q", "HEAD"])
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// Bloque C1: lecturas
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn git_repo_info(state: State<'_, AppState>, repo: Option<String>) -> AppResult<RepoInfo> {
    status2::repo_info(&state, repo.as_deref()).await
}

#[tauri::command]
pub async fn git_status_v2(state: State<'_, AppState>, repo: Option<String>) -> AppResult<StatusV2> {
    status2::status_v2(&state, repo.as_deref()).await
}

#[tauri::command]
pub async fn git_log(state: State<'_, AppState>, repo: Option<String>, query: LogQuery) -> AppResult<LogPage> {
    log::log(&state, repo.as_deref(), query).await
}

#[tauri::command]
pub async fn git_refs(state: State<'_, AppState>, repo: Option<String>) -> AppResult<Refs> {
    refs::refs(&state, repo.as_deref()).await
}

#[tauri::command]
pub async fn git_commit_detail(state: State<'_, AppState>, repo: Option<String>, sha: String) -> AppResult<CommitDetail> {
    log::commit_detail(&state, repo.as_deref(), sha).await
}

#[tauri::command]
pub async fn git_diff(state: State<'_, AppState>, repo: Option<String>, target: DiffTarget, context: u32) -> AppResult<FileDiff> {
    diff::diff(&state, repo.as_deref(), target, context).await
}

#[tauri::command]
pub async fn git_file_at(state: State<'_, AppState>, repo: Option<String>, rev: String, path: String) -> AppResult<String> {
    diff::file_at(&state, repo.as_deref(), rev, path).await
}

#[tauri::command]
pub async fn git_blame(state: State<'_, AppState>, repo: Option<String>, path: String, rev: Option<String>) -> AppResult<BlameResult> {
    blame::blame(&state, repo.as_deref(), path, rev).await
}

#[tauri::command]
pub async fn git_file_history(state: State<'_, AppState>, repo: Option<String>, path: String, skip: u32, limit: u32) -> AppResult<LogPage> {
    log::file_history(&state, repo.as_deref(), path, skip, limit).await
}

// ---------------------------------------------------------------------------
// Bloque C1: operaciones mutadoras (serializadas con `git_op`)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn git_stage(state: State<'_, AppState>, repo: Option<String>, paths: Vec<String>) -> AppResult<OpOutcome> {
    ops::stage(&state, repo.as_deref(), paths).await
}

#[tauri::command]
pub async fn git_unstage(state: State<'_, AppState>, repo: Option<String>, paths: Vec<String>) -> AppResult<OpOutcome> {
    ops::unstage(&state, repo.as_deref(), paths).await
}

#[tauri::command]
pub async fn git_discard(state: State<'_, AppState>, repo: Option<String>, paths: Vec<String>, untracked: bool) -> AppResult<OpOutcome> {
    ops::discard(&state, repo.as_deref(), paths, untracked).await
}

#[tauri::command]
pub async fn git_apply_patch(state: State<'_, AppState>, repo: Option<String>, patch: String, cached: bool, reverse: bool) -> AppResult<OpOutcome> {
    ops::apply_patch(&state, repo.as_deref(), patch, cached, reverse).await
}

#[tauri::command]
pub async fn git_commit(state: State<'_, AppState>, repo: Option<String>, message: String, amend: bool, sign_off: bool) -> AppResult<OpOutcome> {
    ops::commit(&state, repo.as_deref(), message, amend, sign_off).await
}

#[tauri::command]
pub async fn git_branch(state: State<'_, AppState>, repo: Option<String>, action: BranchAction) -> AppResult<OpOutcome> {
    ops::branch(&state, repo.as_deref(), action).await
}

#[tauri::command]
pub async fn git_checkout_commit(state: State<'_, AppState>, repo: Option<String>, sha: String) -> AppResult<OpOutcome> {
    ops::checkout_commit(&state, repo.as_deref(), sha).await
}

#[tauri::command]
pub async fn git_merge(state: State<'_, AppState>, repo: Option<String>, name: String, no_ff: bool, squash: bool) -> AppResult<OpOutcome> {
    ops::merge(&state, repo.as_deref(), name, no_ff, squash).await
}

#[tauri::command]
pub async fn git_rebase(state: State<'_, AppState>, repo: Option<String>, onto: String) -> AppResult<OpOutcome> {
    ops::rebase(&state, repo.as_deref(), onto).await
}

#[tauri::command]
pub async fn git_cherry_pick(state: State<'_, AppState>, repo: Option<String>, shas: Vec<String>) -> AppResult<OpOutcome> {
    ops::cherry_pick(&state, repo.as_deref(), shas).await
}

#[tauri::command]
pub async fn git_revert(state: State<'_, AppState>, repo: Option<String>, sha: String) -> AppResult<OpOutcome> {
    ops::revert(&state, repo.as_deref(), sha).await
}

#[tauri::command]
pub async fn git_reset(state: State<'_, AppState>, repo: Option<String>, sha: String, mode: ResetMode, confirm: Option<bool>) -> AppResult<OpOutcome> {
    ops::reset(&state, repo.as_deref(), sha, mode, confirm.unwrap_or(false)).await
}

#[tauri::command]
pub async fn git_sequence(state: State<'_, AppState>, repo: Option<String>, op: SeqOp, action: SeqAction) -> AppResult<OpOutcome> {
    ops::sequence(&state, repo.as_deref(), op, action).await
}

#[tauri::command]
pub async fn git_conflict(state: State<'_, AppState>, repo: Option<String>, path: String, action: ConflictAction) -> AppResult<OpOutcome> {
    ops::conflict(&state, repo.as_deref(), path, action).await
}

#[tauri::command]
pub async fn git_stash(state: State<'_, AppState>, repo: Option<String>, action: StashAction) -> AppResult<OpOutcome> {
    ops::stash(&state, repo.as_deref(), action).await
}

#[tauri::command]
pub async fn git_tag(state: State<'_, AppState>, repo: Option<String>, action: TagAction) -> AppResult<OpOutcome> {
    ops::tag(&state, repo.as_deref(), action).await
}

#[tauri::command]
pub async fn git_init(state: State<'_, AppState>, path: String) -> AppResult<OpOutcome> {
    ops::init(&state, path).await
}

// ---------------------------------------------------------------------------
// Bloque C1: remotos con progreso por Channel
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn git_remote(
    state: State<'_, AppState>,
    repo: Option<String>,
    op: RemoteOp,
    job_id: String,
    on_progress: Channel<GitProgress>,
) -> AppResult<OpOutcome> {
    remote::run_remote(&state, repo.as_deref(), op, job_id, move |ev| on_progress.send(ev).is_ok()).await
}

#[tauri::command]
pub async fn git_cancel(state: State<'_, AppState>, job_id: String) -> AppResult<bool> {
    Ok(remote::cancel(&state, &job_id))
}

// ---------------------------------------------------------------------------
// Multi-repo: detección y resumen
// ---------------------------------------------------------------------------

/// Detecta los repos de todas las raíces (profundidad <= 3) y los cachea.
#[tauri::command]
pub async fn git_repos(state: State<'_, AppState>) -> AppResult<ReposResult> {
    repos::detect_and_cache(&state).await
}

#[tauri::command]
pub async fn git_repo_summary(state: State<'_, AppState>, repo: String) -> AppResult<RepoSummary> {
    repos::summary(&state, &repo).await
}

/// Resumen de todos los repos detectados (semáforo de 4). Un repo que falla no aborta el resto.
#[tauri::command]
pub async fn git_repos_summary(state: State<'_, AppState>) -> AppResult<Vec<RepoSummary>> {
    repos::summary_all(&state).await
}
