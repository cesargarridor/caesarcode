mod agents;
mod app_cmds;
mod env_path;
mod error;
mod external;
mod fs;
mod git;
mod lsp;
mod projects;
mod run;
mod services;
mod state;
mod terminal;
mod util;
mod viewer;
mod workspace;

use tauri::{Manager, RunEvent};

use crate::state::AppState;

/// Menú de macOS sin "Cerrar ventana" (Cmd+W lo gestiona el editor). Conserva Edición para copiar/pegar.
#[cfg(target_os = "macos")]
fn macos_menu(handle: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{Menu, PredefinedMenuItem as P, Submenu};

    let app = Submenu::with_items(
        handle,
        "CaesarCode",
        true,
        &[
            &P::about(handle, None, None)?,
            &P::separator(handle)?,
            &P::services(handle, None)?,
            &P::separator(handle)?,
            &P::hide(handle, None)?,
            &P::hide_others(handle, None)?,
            &P::separator(handle)?,
            &P::quit(handle, None)?,
        ],
    )?;
    let edit = Submenu::with_items(
        handle,
        "Edit",
        true,
        &[
            &P::undo(handle, None)?,
            &P::redo(handle, None)?,
            &P::separator(handle)?,
            &P::cut(handle, None)?,
            &P::copy(handle, None)?,
            &P::paste(handle, None)?,
            &P::select_all(handle, None)?,
        ],
    )?;
    let window = Submenu::with_items(handle, "Window", true, &[&P::minimize(handle, None)?, &P::maximize(handle, None)?])?;
    Menu::with_items(handle, &[&app, &edit, &window])
}

pub fn run() {
    // Antes de crear nada: PATH del shell de login (git, npm, servidores LSP...).
    env_path::fix_path();

    let builder = tauri::Builder::default();
    // macOS: el menú por defecto liga Cmd+W a "Cerrar ventana"; aquí Cmd+W cierra la pestaña.
    #[cfg(target_os = "macos")]
    let builder = builder.menu(macos_menu);
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::new())
        .setup(|app| {
            // Archivos pasados por línea de comandos (Windows/Linux "Abrir con").
            let paths = external::cli_paths();
            if !paths.is_empty() {
                let handle = app.handle().clone();
                // La webview aún no escucha: se emite tras un instante.
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    external::grant_and_emit(&handle, paths);
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                external::grant_and_emit(window.app_handle(), paths.clone());
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_cmds::app_info,
            workspace::commands::workspace_open,
            workspace::commands::workspace_current,
            workspace::commands::workspace_roots,
            workspace::commands::workspace_add_root,
            workspace::commands::workspace_remove_root,
            fs::commands::fs_read_dir,
            fs::commands::fs_read_file,
            fs::commands::fs_write_file,
            fs::commands::fs_create_file,
            fs::commands::fs_create_dir,
            fs::commands::fs_rename,
            fs::commands::fs_delete,
            fs::commands::fs_watch_subscribe,
            fs::commands::fs_classify,
            git::commands::git_status,
            git::commands::git_repo_info,
            git::commands::git_status_v2,
            git::commands::git_log,
            git::commands::git_refs,
            git::commands::git_commit_detail,
            git::commands::git_diff,
            git::commands::git_file_at,
            git::commands::git_stage,
            git::commands::git_unstage,
            git::commands::git_discard,
            git::commands::git_apply_patch,
            git::commands::git_commit,
            git::commands::git_branch,
            git::commands::git_checkout_commit,
            git::commands::git_merge,
            git::commands::git_rebase,
            git::commands::git_cherry_pick,
            git::commands::git_revert,
            git::commands::git_reset,
            git::commands::git_sequence,
            git::commands::git_conflict,
            git::commands::git_stash,
            git::commands::git_tag,
            git::commands::git_remote,
            git::commands::git_cancel,
            git::commands::git_blame,
            git::commands::git_file_history,
            git::commands::git_init,
            git::commands::git_repos,
            git::commands::git_repo_summary,
            git::commands::git_repos_summary,
            fs::commands::quick_open,
            fs::commands::search_global,
            services::commands::services_load,
            services::commands::services_save,
            services::commands::services_detect,
            services::commands::services_subscribe,
            services::commands::service_start,
            services::commands::service_stop,
            services::commands::service_restart,
            services::commands::services_start_all,
            services::commands::services_stop_all,
            services::commands::services_status,
            services::commands::service_logs,
            services::commands::service_clear_logs,
            services::commands::port_kill,
            run::commands::run_configs_load,
            run::commands::run_configs_save,
            run::commands::run_templates,
            run::commands::run_detect,
            run::commands::run_resolve_file,
            run::commands::run_start,
            terminal::commands::terminal_open,
            terminal::commands::terminal_write,
            terminal::commands::terminal_resize,
            terminal::commands::terminal_close,
            lsp::commands::lsp_subscribe,
            lsp::commands::lsp_did_open,
            lsp::commands::lsp_did_change,
            lsp::commands::lsp_did_save,
            lsp::commands::lsp_did_close,
            lsp::commands::lsp_completion,
            lsp::commands::lsp_hover,
            lsp::commands::lsp_definition,
            lsp::commands::lsp_status,
            projects::commands::projects_recent_list,
            projects::commands::projects_recent_remove,
            projects::commands::projects_recent_pin,
            projects::commands::projects_tools_detect,
            projects::commands::project_generators,
            projects::commands::spring_catalog,
            projects::commands::project_create,
            projects::commands::project_cancel,
            agents::commands::agents_detect,
            agents::commands::agent_open,
            external::external_open_dialog,
            viewer::commands::read_tabular_file,
            viewer::commands::media_allow,
        ])
        .build(tauri::generate_context!())
        .expect("error al construir la aplicación Tauri")
        .run(|app, event| {
            match event {
                RunEvent::Exit => app.state::<AppState>().shutdown(),
                // macOS: "Abrir con CaesarCode" / doble clic en un archivo asociado.
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                RunEvent::Opened { urls } => {
                    let paths: Vec<std::path::PathBuf> = urls.iter().filter_map(|u| u.to_file_path().ok()).collect();
                    external::grant_and_emit(app, paths);
                }
                _ => {}
            }
        });
}
