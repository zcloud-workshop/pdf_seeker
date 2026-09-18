use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub s3: Option<S3Config>,
    pub ocr: Option<OcrConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrConfig {
    pub model_dir: String,
    pub det_model: String,
    pub rec_model: String,
    pub keys_file: String,
    pub language: String,
    pub gpu_enabled: bool,
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

#[derive(Clone, Serialize, Deserialize)]
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

impl std::fmt::Debug for S3Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Config")
            .field("auth_mode", &self.auth_mode)
            .field("endpoint", &self.endpoint)
            .field("region", &self.region)
            .field("bucket", &self.bucket)
            .field("access_key", &self.access_key)
            .field("secret_key", &"[REDACTED]")
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("force_path_style", &self.force_path_style)
            .field("root_prefix", &self.root_prefix)
            .field("max_versions", &self.max_versions)
            .field("version_ttl_days", &self.version_ttl_days)
            .finish()
    }
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
            ocr: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_offline() {
        let cfg = AppConfig::default();
        assert!(
            cfg.s3.is_none(),
            "Default config must not configure remote storage (offline-first)"
        );
        assert_eq!(cfg.general.language, "zh");
        assert_eq!(cfg.general.theme, "system");
    }

    #[test]
    fn test_config_roundtrip_toml() {
        let mut cfg = AppConfig::default();
        cfg.general.default_export_dir = Some("/path/to/export".to_string());
        cfg.s3 = Some(S3Config {
            auth_mode: S3AuthMode::Static,
            endpoint: "https://s3.example.com".to_string(),
            region: "us-west-2".to_string(),
            bucket: "my-bucket".to_string(),
            access_key: "AKIA_TEST_KEY".to_string(),
            secret_key: "SUPER_SECRET_VALUE".to_string(),
            session_token: Some("TOKEN_SECRET".to_string()),
            force_path_style: true,
            root_prefix: Some("documents".to_string()),
            max_versions: Some(10),
            version_ttl_days: Some(30),
        });

        let toml_str = toml::to_string_pretty(&cfg).expect("Failed to serialize toml");
        let restored: AppConfig = toml::from_str(&toml_str).expect("Failed to deserialize toml");

        assert_eq!(
            restored.general.default_export_dir,
            Some("/path/to/export".to_string())
        );
        let s3 = restored.s3.expect("s3 config must be preserved");
        assert_eq!(s3.bucket, "my-bucket");
        assert_eq!(s3.access_key, "AKIA_TEST_KEY");
        assert_eq!(s3.secret_key, "SUPER_SECRET_VALUE");
        assert_eq!(s3.session_token, Some("TOKEN_SECRET".to_string()));
    }

    #[test]
    fn test_s3_credentials_never_printed_in_debug() {
        let s3 = S3Config {
            auth_mode: S3AuthMode::Static,
            endpoint: "https://s3.example.com".to_string(),
            region: "us-west-2".to_string(),
            bucket: "my-bucket".to_string(),
            access_key: "AKIA_VISIBLE".to_string(),
            secret_key: "TOP_SECRET_NEVER_LOG".to_string(),
            session_token: Some("CONFIDENTIAL_SESSION_TOKEN".to_string()),
            force_path_style: true,
            root_prefix: None,
            max_versions: None,
            version_ttl_days: None,
        };

        let debug_str = format!("{:?}", s3);
        assert!(
            !debug_str.contains("TOP_SECRET_NEVER_LOG"),
            "Secret key must be redacted"
        );
        assert!(
            !debug_str.contains("CONFIDENTIAL_SESSION_TOKEN"),
            "Session token must be redacted"
        );
        assert!(
            debug_str.contains("[REDACTED]"),
            "Redacted placeholder must be present"
        );
        assert!(
            debug_str.contains("AKIA_VISIBLE"),
            "Access key id may remain visible"
        );
    }
}
