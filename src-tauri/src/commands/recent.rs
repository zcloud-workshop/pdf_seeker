use crate::error::{AppError, AppResult};
use std::sync::Mutex;
use std::sync::PoisonError;
use tauri::State;

pub struct RecentFilesState(pub Mutex<Vec<String>>);

fn lock_err(e: PoisonError<std::sync::MutexGuard<'_, Vec<String>>>) -> AppError {
    AppError::Config(e.to_string())
}

#[tauri::command]
pub fn get_recent_files(state: State<'_, RecentFilesState>) -> AppResult<Vec<String>> {
    let files = state.0.lock().map_err(lock_err)?;
    Ok(files.clone())
}

#[tauri::command]
pub fn add_recent_file(state: State<'_, RecentFilesState>, path: String) -> AppResult<Vec<String>> {
    let mut files = state.0.lock().map_err(lock_err)?;
    files.retain(|f| f != &path);
    files.insert(0, path);
    if files.len() > 20 {
        files.truncate(20);
    }
    Ok(files.clone())
}

#[tauri::command]
pub fn clear_recent_files(state: State<'_, RecentFilesState>) -> AppResult<()> {
    let mut files = state.0.lock().map_err(lock_err)?;
    files.clear();
    Ok(())
}
