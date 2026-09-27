use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub s3: Option<S3Config>,
    #[serde(default)]
    pub recovery_notice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub language: String,
    pub theme: String,
    pub default_export_dir: Option<String>,
    pub recent_files_max: usize,
    #[serde(default)]
    pub recent_files: Vec<String>,
    /// Check for app updates on startup
    #[serde(default = "default_true")]
    pub auto_update_check: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum S3AuthMode {
    /// No authentication (public buckets)
    None,
    /// Static access key + secret key
    #[default]
    Static,
    /// Load credentials from environment / instance profile (EC2, ECS, etc.)
    Env,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    pub auth_mode: S3AuthMode,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub session_token: Option<String>,
    pub force_path_style: bool,
    pub root_prefix: Option<String>,
    pub max_versions: Option<usize>,
    pub version_ttl_days: Option<u64>,
    /// Auto-upload a config backup to S3 whenever settings are saved
    #[serde(default)]
    pub auto_backup_config: bool,
    /// Epoch seconds of the last successful config backup
    #[serde(default)]
    pub last_backup_at: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                language: "zh".to_string(),
                theme: "system".to_string(),
                default_export_dir: None,
                recent_files_max: 20,
                recent_files: Vec::new(),
                auto_update_check: true,
            },
            s3: None,
            recovery_notice: None,
        }
    }
}

static CONFIG_FILE_NAME: &str = "config.toml";
static CONFIG_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn config_dir(handle: &tauri::AppHandle) -> AppResult<PathBuf> {
    let path = handle
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Config(e.to_string()))?;
    if !path.exists() {
        fs::create_dir_all(&path)?;
    }
    Ok(path)
}

fn config_file_path(handle: &tauri::AppHandle) -> AppResult<PathBuf> {
    Ok(config_dir(handle)?.join(CONFIG_FILE_NAME))
}

pub fn load_config_with_handle(handle: &tauri::AppHandle) -> AppResult<AppConfig> {
    let path = config_file_path(handle)?;
    if !path.exists() {
        let default = AppConfig::default();
        save_config_with_handle(handle, &default)?;
        return Ok(default);
    }
    let content = fs::read_to_string(&path)?;
    let config: AppConfig = toml::from_str(&content)?;
    Ok(config)
}

pub fn save_config_with_handle(handle: &tauri::AppHandle, config: &AppConfig) -> AppResult<()> {
    let path = config_file_path(handle)?;
    let content = toml::to_string_pretty(config)?;
    replace_atomically(&path, content.as_bytes())
}

fn replace_atomically(path: &Path, content: &[u8]) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Config("Config path has no parent directory".into()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(CONFIG_FILE_NAME);
    let temp_path = (0..32)
        .find_map(|_| {
            let sequence = CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let candidate = parent.join(format!(
                ".{file_name}.{}.{}.tmp",
                std::process::id(),
                sequence
            ));
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(mut file) => {
                    if let Err(error) = file.write_all(content).and_then(|_| file.sync_all()) {
                        let _ = fs::remove_file(&candidate);
                        return Some(Err(error));
                    }
                    drop(file);
                    Some(Ok(candidate))
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .ok_or_else(|| AppError::Config("Could not allocate a temporary config file".into()))??;

    if let Err(error) = fs::rename(&temp_path, &path) {
        let _ = fs::remove_file(&temp_path);
        return Err(AppError::Io(error));
    }
    Ok(())
}

fn preserve_invalid_config(path: &Path) -> AppResult<PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    for _ in 0..32 {
        let sequence = CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let backup = path.with_file_name(format!(
            "{CONFIG_FILE_NAME}.corrupt-{}-{stamp}-{sequence}",
            std::process::id()
        ));
        match fs::rename(path, &backup) {
            Ok(()) => return Ok(backup),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(AppError::Io(error)),
        }
    }
    Err(AppError::Config(
        "Could not preserve the invalid config file".into(),
    ))
}

pub fn commit_config_with_handle(
    handle: &tauri::AppHandle,
    current: &mut AppConfig,
    candidate: AppConfig,
) -> AppResult<()> {
    save_config_with_handle(handle, &candidate)?;
    *current = candidate;
    Ok(())
}

pub fn init(handle: &tauri::AppHandle, state: &Mutex<AppConfig>) -> AppResult<()> {
    let loaded = match load_config_with_handle(handle) {
        Ok(config) => config,
        Err(AppError::Toml(error)) => {
            let path = config_file_path(handle)?;
            let backup = preserve_invalid_config(&path).map_err(|rename_error| {
                AppError::Config(format!(
                    "Config TOML is invalid ({error}); could not preserve it at {}: {rename_error}",
                    path.display()
                ))
            })?;
            let mut config = AppConfig::default();
            config.recovery_notice = Some(backup.display().to_string());
            save_config_with_handle(handle, &config)?;
            config
        }
        Err(error) => return Err(error),
    };
    let mut cfg = state.lock().map_err(|e| AppError::Config(e.to_string()))?;
    *cfg = loaded;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_atomically_replaces_contents_without_leaving_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        fs::write(&path, "old config").unwrap();

        replace_atomically(&path, b"new config").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new config");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn invalid_config_is_preserved_for_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        fs::write(&path, "not valid toml").unwrap();

        let backup = preserve_invalid_config(&path).unwrap();

        assert!(!path.exists());
        assert_eq!(fs::read_to_string(backup).unwrap(), "not valid toml");
    }
}
