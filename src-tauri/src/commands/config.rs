use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use std::sync::Mutex;
use tauri::{AppHandle, State};

fn lock_err<T>(e: std::sync::PoisonError<std::sync::MutexGuard<'_, T>>) -> AppError {
    AppError::Config(e.to_string())
}

#[tauri::command]
pub fn get_config(config: State<'_, Mutex<AppConfig>>) -> AppResult<AppConfig> {
    let cfg = config.lock().map_err(lock_err)?;
    Ok(cfg.clone())
}

#[tauri::command]
pub fn update_config(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    new_config: AppConfig,
) -> AppResult<()> {
    let mut cfg = config.lock().map_err(lock_err)?;
    crate::config::commit_config_with_handle(&app, &mut cfg, new_config)
}

#[tauri::command]
pub fn set_theme_preference(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    theme: String,
) -> AppResult<AppConfig> {
    if !matches!(theme.as_str(), "system" | "light" | "dark") {
        return Err(AppError::Config("Invalid theme preference".into()));
    }
    let mut cfg = config.lock().map_err(lock_err)?;
    let mut candidate = cfg.clone();
    candidate.general.theme = theme;
    crate::config::commit_config_with_handle(&app, &mut cfg, candidate.clone())?;
    Ok(candidate)
}
