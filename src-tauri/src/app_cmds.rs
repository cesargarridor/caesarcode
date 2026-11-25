use serde::Serialize;

use crate::error::AppResult;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub platform: &'static str,
    pub path_sep: String,
}

#[tauri::command]
pub fn app_info() -> AppResult<AppInfo> {
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform,
        path_sep: std::path::MAIN_SEPARATOR.to_string(),
    })
}
