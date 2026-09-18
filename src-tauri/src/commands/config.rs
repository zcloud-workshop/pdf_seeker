use crate::config::AppConfig;
use crate::error::AppResult;
use std::sync::Mutex;
use tauri::State;

#[tauri::command]
pub fn get_config(config: State<'_, Mutex<AppConfig>>) -> AppResult<AppConfig> {
    let cfg = config
        .lock()
        .map_err(|e| crate::error::AppError::Config(e.to_string()))?;
    Ok(cfg.clone())
}

#[tauri::command]
pub fn update_config(
    app_handle: tauri::AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    new_config: AppConfig,
) -> AppResult<()> {
    let mut cfg = config
        .lock()
        .map_err(|e| crate::error::AppError::Config(e.to_string()))?;
    crate::config::save_config_with_handle(&app_handle, &new_config)?;
    *cfg = new_config;
    Ok(())
}
