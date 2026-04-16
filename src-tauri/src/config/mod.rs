use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub s3: Option<S3Config>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub language: String,
    pub theme: String,
    pub default_export_dir: Option<String>,
    pub recent_files_max: usize,
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
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                language: "zh".to_string(),
                theme: "system".to_string(),
                default_export_dir: None,
                recent_files_max: 20,
            },
            s3: None,
        }
    }
}

static CONFIG_FILE_NAME: &str = "config.toml";

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

pub fn load_config() -> AppConfig {
    AppConfig::default()
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

pub fn save_config(config: &AppConfig) -> AppResult<()> {
    let content = toml::to_string_pretty(config)?;
    // Write to default location when no handle available
    let home = directories::ProjectDirs::from("com", "pdfseeker", "PDF Seeker")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    if !home.exists() {
        fs::create_dir_all(&home)?;
    }
    fs::write(home.join(CONFIG_FILE_NAME), content)?;
    Ok(())
}

pub fn save_config_with_handle(handle: &tauri::AppHandle, config: &AppConfig) -> AppResult<()> {
    let path = config_file_path(handle)?;
    let content = toml::to_string_pretty(config)?;
    fs::write(&path, content)?;
    Ok(())
}

pub fn init(handle: &tauri::AppHandle) -> AppResult<()> {
    load_config_with_handle(handle)?;
    Ok(())
}
