use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use std::sync::Mutex;
use tauri::{AppHandle, State};

fn lock_err<T>(e: std::sync::PoisonError<std::sync::MutexGuard<'_, T>>) -> AppError {
    AppError::Config(e.to_string())
}

#[tauri::command]
pub fn get_recent_files(config: State<'_, Mutex<AppConfig>>) -> AppResult<Vec<String>> {
    let cfg = config.lock().map_err(lock_err)?;
    Ok(cfg.general.recent_files.clone())
}

#[tauri::command]
pub fn add_recent_file(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    path: String,
) -> AppResult<Vec<String>> {
    let mut cfg = config.lock().map_err(lock_err)?;
    let max = cfg.general.recent_files_max;
    let files = &mut cfg.general.recent_files;
    files.retain(|f| f != &path);
    files.insert(0, path);
    files.truncate(max);
    let result = files.clone();
    crate::config::save_config_with_handle(&app, &cfg)?;
    Ok(result)
}

#[tauri::command]
pub fn clear_recent_files(app: AppHandle, config: State<'_, Mutex<AppConfig>>) -> AppResult<()> {
    let mut cfg = config.lock().map_err(lock_err)?;
    cfg.general.recent_files.clear();
    crate::config::save_config_with_handle(&app, &cfg)?;
    Ok(())
}
