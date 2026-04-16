use crate::config::AppConfig;
use crate::error::AppResult;
use tauri::State;
use std::sync::Mutex;

#[tauri::command]
pub fn get_config(config: State<'_, Mutex<AppConfig>>) -> AppResult<AppConfig> {
    let cfg = config.lock().map_err(|e| crate::error::AppError::Config(e.to_string()))?;
    Ok(cfg.clone())
}

#[tauri::command]
pub fn update_config(config: State<'_, Mutex<AppConfig>>, new_config: AppConfig) -> AppResult<()> {
    let mut cfg = config.lock().map_err(|e| crate::error::AppError::Config(e.to_string()))?;
    crate::config::save_config(&new_config)?;
    *cfg = new_config;
    Ok(())
}
