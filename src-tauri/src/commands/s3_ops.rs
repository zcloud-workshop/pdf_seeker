use aws_sdk_s3::primitives::ByteStream;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::{AppConfig, S3Config};
use crate::error::AppError;
use crate::error::AppResult;

async fn build_client(cfg: &S3Config) -> AppResult<aws_sdk_s3::Client> {
    let region = if cfg.region.is_empty() {
        "us-east-1".to_string()
    } else {
        cfg.region.clone()
    };

    match &cfg.auth_mode {
        crate::config::S3AuthMode::None => {
            let mut builder = aws_sdk_s3::config::Builder::new()
                .behavior_version(aws_config::BehaviorVersion::latest())
                .region(aws_sdk_s3::config::Region::new(region.clone()))
                .force_path_style(cfg.force_path_style)
                .allow_no_auth();
            if !cfg.endpoint.is_empty() {
                builder = builder.endpoint_url(&cfg.endpoint);
            }
            Ok(aws_sdk_s3::Client::from_conf(builder.build()))
        }
        crate::config::S3AuthMode::Static => {
            let credentials = aws_sdk_s3::config::Credentials::new(
                &cfg.access_key,
                &cfg.secret_key,
                cfg.session_token.clone(),
                None,
                "pdf-seeker-static",
            );
            let mut builder = aws_sdk_s3::config::Builder::new()
                .behavior_version(aws_config::BehaviorVersion::latest())
                .region(aws_sdk_s3::config::Region::new(region))
                .force_path_style(cfg.force_path_style)
                .credentials_provider(aws_sdk_s3::config::SharedCredentialsProvider::new(
                    credentials,
                ));
            if !cfg.endpoint.is_empty() {
                builder = builder.endpoint_url(&cfg.endpoint);
            }
            Ok(aws_sdk_s3::Client::from_conf(builder.build()))
        }
        crate::config::S3AuthMode::Env => {
            let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
                .load()
                .await;
            let mut builder = aws_sdk_s3::config::Builder::from(&sdk_config)
                .region(aws_sdk_s3::config::Region::new(region))
                .force_path_style(cfg.force_path_style);
            if !cfg.endpoint.is_empty() {
                builder = builder.endpoint_url(&cfg.endpoint);
            }
            Ok(aws_sdk_s3::Client::from_conf(builder.build()))
        }
    }
}

fn prefix(cfg: &S3Config) -> String {
    cfg.root_prefix
        .as_deref()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string()
}

fn folder_prefix(cfg: &S3Config, folder: &str) -> String {
    let root = prefix(cfg);
    let folder = folder.trim_matches('/');
    match (root.is_empty(), folder.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("{folder}/"),
        (false, true) => format!("{root}/"),
        (false, false) => format!("{root}/{folder}/"),
    }
}

fn folder_object_key(cfg: &S3Config, folder: &str) -> String {
    folder_prefix(cfg, folder)
}

static DOWNLOAD_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempDownload {
    path: PathBuf,
    file: Option<File>,
    committed: bool,
}

impl TempDownload {
    fn create(target: &Path) -> AppResult<Self> {
        let parent = target
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let name = target
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        for _ in 0..32 {
            let sequence = DOWNLOAD_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".{name}.pdf-seeker-{}-{sequence}.tmp",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                        committed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(AppError::Io(error)),
            }
        }
        Err(AppError::Config(
            "Could not allocate a temporary download file".into(),
        ))
    }
}

impl Drop for TempDownload {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3FileItem {
    pub key: String,
    pub name: String,
    pub size: u64,
    pub last_modified: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3ListResult {
    pub items: Vec<S3FileItem>,
    pub prefixes: Vec<String>,
    pub common_prefixes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3VersionItem {
    pub version_id: String,
    pub size: u64,
    pub last_modified: String,
    pub is_latest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3VersionsResult {
    pub versions: Vec<S3VersionItem>,
    pub delete_markers: Vec<String>,
}

async fn list_versions_for_key(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
) -> AppResult<S3VersionsResult> {
    let mut versions = Vec::new();
    let mut delete_markers = Vec::new();
    let mut key_marker: Option<String> = None;
    let mut version_marker: Option<String> = None;
    loop {
        let mut request = client
            .list_object_versions()
            .bucket(bucket)
            .prefix(key)
            .max_keys(1000);
        if let Some(marker) = key_marker.as_deref() {
            request = request.key_marker(marker);
        }
        if let Some(marker) = version_marker.as_deref() {
            request = request.version_id_marker(marker);
        }
        let response = request
            .send()
            .await
            .map_err(|error| AppError::S3(error.to_string()))?;

        for version in response.versions() {
            if version.key() != Some(key) {
                continue;
            }
            versions.push(S3VersionItem {
                version_id: version.version_id().unwrap_or("null").to_string(),
                size: version.size().unwrap_or(0) as u64,
                last_modified: version
                    .last_modified()
                    .map(|time| time.to_string())
                    .unwrap_or_default(),
                is_latest: version.is_latest().unwrap_or(false),
            });
        }
        for marker in response.delete_markers() {
            if marker.key() == Some(key) {
                delete_markers.push(marker.version_id().unwrap_or("null").to_string());
            }
        }

        if !response.is_truncated().unwrap_or(false) {
            break;
        }
        let next_key_marker = response.next_key_marker().map(str::to_string);
        let next_version_marker = response.next_version_id_marker().map(str::to_string);
        if next_key_marker == key_marker && next_version_marker == version_marker {
            return Err(AppError::S3(
                "S3 version listing did not advance its pagination markers".into(),
            ));
        }
        key_marker = next_key_marker;
        version_marker = next_version_marker;
    }
    versions.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    Ok(S3VersionsResult {
        versions,
        delete_markers,
    })
}

#[tauri::command]
pub async fn s3_test_connection(s3_config: S3Config) -> AppResult<bool> {
    let client = build_client(&s3_config).await?;
    client
        .head_bucket()
        .bucket(&s3_config.bucket)
        .send()
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;
    Ok(true)
}

#[tauri::command]
pub async fn s3_list_files(s3_config: S3Config, folder: String) -> AppResult<S3ListResult> {
    let client = build_client(&s3_config).await?;
    let full_prefix = folder_prefix(&s3_config, &folder);

    let mut items = Vec::new();
    let mut common_prefixes = Vec::new();
    let mut continuation_token = None;

    loop {
        let mut req = client
            .list_objects_v2()
            .bucket(&s3_config.bucket)
            .prefix(&full_prefix)
            .delimiter("/");

        if let Some(token) = continuation_token {
            req = req.continuation_token(token);
        }

        let resp = req.send().await.map_err(|e| AppError::S3(e.to_string()))?;

        if let Some(prefixes) = resp.common_prefixes {
            for p in prefixes {
                let pfx = p.prefix.unwrap_or_default();
                let name = pfx
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or(&pfx)
                    .to_string();
                let key_owned = pfx.clone();
                common_prefixes.push(pfx);
                items.push(S3FileItem {
                    key: key_owned,
                    name,
                    size: 0,
                    last_modified: String::new(),
                    is_dir: true,
                });
            }
        }

        if let Some(contents) = resp.contents {
            for obj in contents {
                let key = obj.key.unwrap_or_default();
                if key.ends_with('/') {
                    continue;
                }
                let name = key.strip_prefix(&full_prefix).unwrap_or(&key).to_string();
                let name = name.trim_end_matches('/').to_string();
                if name.is_empty() {
                    continue;
                }
                let size = obj.size.unwrap_or(0) as u64;
                let last_modified = obj.last_modified.map(|t| t.to_string()).unwrap_or_default();
                items.push(S3FileItem {
                    key,
                    name,
                    size,
                    last_modified,
                    is_dir: false,
                });
            }
        }

        continuation_token = resp.next_continuation_token;
        if continuation_token.is_none() {
            break;
        }
    }

    Ok(S3ListResult {
        items,
        prefixes: Vec::new(),
        common_prefixes,
    })
}

#[tauri::command]
pub async fn s3_upload_file(
    s3_config: S3Config,
    local_path: String,
    folder: String,
) -> AppResult<()> {
    let client = build_client(&s3_config).await?;
    let file_name = Path::new(&local_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "upload".to_string());
    let final_key = format!("{}{file_name}", folder_prefix(&s3_config, &folder));

    let body = ByteStream::from_path(&local_path)
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;

    client
        .put_object()
        .bucket(&s3_config.bucket)
        .key(&final_key)
        .body(body)
        .send()
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub async fn s3_download_file(
    s3_config: S3Config,
    remote_key: String,
    local_path: String,
) -> AppResult<()> {
    let client = build_client(&s3_config).await?;
    let resp = client
        .get_object()
        .bucket(&s3_config.bucket)
        .key(&remote_key)
        .send()
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;
    let target = Path::new(&local_path);
    if let Some(parent) = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut temp = TempDownload::create(target)?;
    let mut body = resp.body;
    while let Some(chunk) = body
        .try_next()
        .await
        .map_err(|error| AppError::S3(error.to_string()))?
    {
        temp.file
            .as_mut()
            .ok_or_else(|| AppError::Config("Download temp file is closed".into()))?
            .write_all(&chunk)?;
    }
    let file = temp
        .file
        .take()
        .ok_or_else(|| AppError::Config("Download temp file is closed".into()))?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temp.path, target)?;
    temp.committed = true;

    Ok(())
}

#[tauri::command]
pub async fn s3_delete_file(s3_config: S3Config, remote_key: String) -> AppResult<()> {
    let client = build_client(&s3_config).await?;
    client
        .delete_object()
        .bucket(&s3_config.bucket)
        .key(&remote_key)
        .send()
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub async fn s3_list_versions(
    s3_config: S3Config,
    remote_key: String,
) -> AppResult<S3VersionsResult> {
    let client = build_client(&s3_config).await?;
    let mut result = list_versions_for_key(&client, &s3_config.bucket, &remote_key).await?;
    let display_limit = s3_config
        .max_versions
        .filter(|limit| *limit > 0)
        .unwrap_or(20);
    result.versions.truncate(display_limit);
    Ok(result)
}

#[tauri::command]
pub async fn s3_delete_version(
    s3_config: S3Config,
    remote_key: String,
    version_id: String,
) -> AppResult<()> {
    let client = build_client(&s3_config).await?;
    client
        .delete_object()
        .bucket(&s3_config.bucket)
        .key(&remote_key)
        .version_id(&version_id)
        .send()
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub async fn s3_create_folder(s3_config: S3Config, folder_name: String) -> AppResult<()> {
    let client = build_client(&s3_config).await?;
    let key = folder_object_key(&s3_config, &folder_name);

    client
        .put_object()
        .bucket(&s3_config.bucket)
        .key(&key)
        .body(ByteStream::from_static(b""))
        .send()
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub async fn s3_get_presigned_url(
    s3_config: S3Config,
    remote_key: String,
    expires_in_secs: u64,
) -> AppResult<String> {
    let client = build_client(&s3_config).await?;
    let presigned = client
        .get_object()
        .bucket(&s3_config.bucket)
        .key(&remote_key)
        .presigned(
            aws_sdk_s3::presigning::PresigningConfig::builder()
                .expires_in(std::time::Duration::from_secs(expires_in_secs))
                .build()?,
        )
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;

    Ok(presigned.uri().to_string())
}

// ==================== Config sync (backup / restore) ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncBackupInfo {
    pub backed_at: String,
    pub app_version: String,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigBackupToml {
    general: crate::config::GeneralConfig,
    s3: Option<S3Config>,
    backup: ConfigBackupMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigBackupMetadata {
    format_version: u32,
    backed_at: String,
    app_version: String,
    checksum: String,
}

#[derive(Serialize)]
struct ConfigBackupPayload<'a> {
    format_version: u32,
    backed_at: &'a str,
    app_version: &'a str,
    config: &'a AppConfig,
}

const CONFIG_BACKUP_FORMAT_VERSION: u32 = 1;

/// Object key for a config-backup file under the user's root prefix
fn sync_key(cfg: &S3Config, file: &str) -> String {
    let base = prefix(cfg);
    if base.is_empty() {
        format!("config-backup/{}", file)
    } else {
        format!("{}/config-backup/{}", base, file)
    }
}

fn backup_safe_config(config: &AppConfig) -> AppConfig {
    let mut safe = config.clone();
    safe.general.recent_files.clear();
    safe.recovery_notice = None;
    if let Some(s3) = safe.s3.as_mut() {
        s3.access_key.clear();
        s3.secret_key.clear();
        s3.session_token = None;
    }
    safe
}

fn backup_checksum(payload: &ConfigBackupPayload<'_>) -> AppResult<String> {
    let bytes = serde_json::to_vec(payload)?;
    Ok(Md5::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn make_config_backup(
    config: &AppConfig,
    backed_at: String,
    app_version: String,
) -> AppResult<(Vec<u8>, SyncBackupInfo)> {
    let config = backup_safe_config(config);
    let payload = ConfigBackupPayload {
        format_version: CONFIG_BACKUP_FORMAT_VERSION,
        backed_at: &backed_at,
        app_version: &app_version,
        config: &config,
    };
    let checksum = backup_checksum(&payload)?;
    let info = SyncBackupInfo {
        backed_at: backed_at.clone(),
        app_version: app_version.clone(),
        checksum: checksum.clone(),
    };
    let backup = ConfigBackupToml {
        general: config.general,
        s3: config.s3,
        backup: ConfigBackupMetadata {
            format_version: CONFIG_BACKUP_FORMAT_VERSION,
            backed_at,
            app_version,
            checksum,
        },
    };
    Ok((toml::to_string_pretty(&backup)?.into_bytes(), info))
}

fn parse_config_backup(bytes: &[u8]) -> AppResult<AppConfig> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| AppError::Config(format!("Backup is not valid UTF-8: {error}")))?;
    let document: toml::Value = toml::from_str(text)?;
    if document.get("backup").is_some() {
        let backup: ConfigBackupToml = document.try_into()?;
        if backup.backup.format_version != CONFIG_BACKUP_FORMAT_VERSION {
            return Err(AppError::Config(format!(
                "Unsupported backup format version {}",
                backup.backup.format_version
            )));
        }
        let config = AppConfig {
            general: backup.general,
            s3: backup.s3,
            recovery_notice: None,
        };
        let payload = ConfigBackupPayload {
            format_version: backup.backup.format_version,
            backed_at: &backup.backup.backed_at,
            app_version: &backup.backup.app_version,
            config: &config,
        };
        if backup_checksum(&payload)? != backup.backup.checksum {
            return Err(AppError::Config(
                "Config backup checksum does not match".into(),
            ));
        }
        return Ok(backup_safe_config(&config));
    }

    let legacy: AppConfig = toml::from_str(text)?;
    Ok(backup_safe_config(&legacy))
}

fn preserve_local_only_data(restored: &mut AppConfig, current: &AppConfig) {
    restored.general.recent_files = current.general.recent_files.clone();
    if let (Some(restored_s3), Some(current_s3)) = (restored.s3.as_mut(), current.s3.as_ref()) {
        if restored_s3.endpoint == current_s3.endpoint
            && restored_s3.region == current_s3.region
            && restored_s3.bucket == current_s3.bucket
        {
            restored_s3.access_key = current_s3.access_key.clone();
            restored_s3.secret_key = current_s3.secret_key.clone();
            restored_s3.session_token = current_s3.session_token.clone();
        }
    }
}

fn same_config_except_backup_time(left: &AppConfig, right: &AppConfig) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    if let Some(s3) = left.s3.as_mut() {
        s3.last_backup_at = None;
    }
    if let Some(s3) = right.s3.as_mut() {
        s3.last_backup_at = None;
    }
    match (toml::to_string(&left), toml::to_string(&right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

#[tauri::command]
pub async fn sync_backup_config(
    app: tauri::AppHandle,
    config: tauri::State<'_, std::sync::Mutex<AppConfig>>,
    s3_config: S3Config,
) -> AppResult<SyncBackupInfo> {
    let backed_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .map_err(|error| AppError::Config(error.to_string()))?;
    let source_config = config
        .lock()
        .map_err(|error| AppError::Config(error.to_string()))?
        .clone();
    let (bytes, info) = make_config_backup(
        &source_config,
        backed_at,
        app.package_info().version.to_string(),
    )?;

    let client = build_client(&s3_config).await?;
    client
        .put_object()
        .bucket(&s3_config.bucket)
        .key(sync_key(&s3_config, "config.toml"))
        .body(ByteStream::from(bytes))
        .send()
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;
    client
        .put_object()
        .bucket(&s3_config.bucket)
        .key(sync_key(&s3_config, "info.json"))
        .body(ByteStream::from(serde_json::to_vec(&info)?))
        .send()
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;

    let mut cfg = config.lock().map_err(|e| AppError::Config(e.to_string()))?;
    let same_bucket = cfg.s3.as_ref().is_some_and(|current| {
        current.endpoint == s3_config.endpoint
            && current.region == s3_config.region
            && current.bucket == s3_config.bucket
    }) && same_config_except_backup_time(&cfg, &source_config);
    if same_bucket {
        let mut candidate = cfg.clone();
        let Some(s3) = candidate.s3.as_mut() else {
            return Ok(info);
        };
        s3.last_backup_at = Some(info.backed_at.clone());
        crate::config::commit_config_with_handle(&app, &mut cfg, candidate)?;
    }

    Ok(info)
}

#[tauri::command]
pub async fn sync_fetch_backup_info(s3_config: S3Config) -> AppResult<Option<SyncBackupInfo>> {
    let client = build_client(&s3_config).await?;
    match client
        .get_object()
        .bucket(&s3_config.bucket)
        .key(sync_key(&s3_config, "info.json"))
        .send()
        .await
    {
        Ok(resp) => {
            let data = resp
                .body
                .collect()
                .await
                .map_err(|e| AppError::S3(e.to_string()))?;
            let info: SyncBackupInfo = serde_json::from_slice(&data.into_bytes())?;
            Ok(Some(info))
        }
        Err(e) => {
            let svc = e.into_service_error();
            if svc.is_no_such_key() {
                Ok(None)
            } else {
                Err(AppError::S3(svc.to_string()))
            }
        }
    }
}

#[tauri::command]
pub async fn sync_list_backup_versions(s3_config: S3Config) -> AppResult<Vec<S3VersionItem>> {
    let client = build_client(&s3_config).await?;
    let key = sync_key(&s3_config, "config.toml");
    let mut versions = list_versions_for_key(&client, &s3_config.bucket, &key)
        .await?
        .versions;
    let display_limit = s3_config
        .max_versions
        .filter(|limit| *limit > 0)
        .unwrap_or(20);
    versions.truncate(display_limit);
    Ok(versions)
}

#[tauri::command]
pub async fn sync_restore_config(
    app: tauri::AppHandle,
    config: tauri::State<'_, std::sync::Mutex<AppConfig>>,
    s3_config: S3Config,
    version_id: Option<String>,
) -> AppResult<AppConfig> {
    let client = build_client(&s3_config).await?;
    let mut req = client
        .get_object()
        .bucket(&s3_config.bucket)
        .key(sync_key(&s3_config, "config.toml"));
    if let Some(v) = version_id.as_deref() {
        if !v.is_empty() {
            req = req.version_id(v);
        }
    }
    let resp = req.send().await.map_err(|e| AppError::S3(e.to_string()))?;
    let data = resp
        .body
        .collect()
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;
    let mut restored = parse_config_backup(&data.into_bytes())?;
    let mut cfg = config.lock().map_err(|e| AppError::Config(e.to_string()))?;
    preserve_local_only_data(&mut restored, &cfg);
    crate::config::commit_config_with_handle(&app, &mut cfg, restored.clone())?;
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GeneralConfig, S3AuthMode};

    fn test_s3_config(root_prefix: Option<&str>) -> S3Config {
        S3Config {
            auth_mode: S3AuthMode::Static,
            endpoint: "http://localhost:9000".into(),
            region: "us-east-1".into(),
            bucket: "test-bucket".into(),
            access_key: "ak".into(),
            secret_key: "sk".into(),
            session_token: None,
            force_path_style: true,
            root_prefix: root_prefix.map(String::from),
            max_versions: None,
            version_ttl_days: None,
            auto_backup_config: false,
            last_backup_at: None,
        }
    }

    #[test]
    fn test_sync_key_respects_root_prefix() {
        assert_eq!(
            sync_key(&test_s3_config(None), "config.toml"),
            "config-backup/config.toml"
        );
        assert_eq!(
            sync_key(&test_s3_config(Some("pdf")), "config.toml"),
            "pdf/config-backup/config.toml"
        );
        assert_eq!(
            sync_key(&test_s3_config(Some("pdf/")), "info.json"),
            "pdf/config-backup/info.json"
        );
    }

    #[test]
    fn folder_prefix_applies_root_once_and_keeps_full_list_keys_compatible() {
        assert_eq!(folder_prefix(&test_s3_config(None), ""), "");
        assert_eq!(folder_prefix(&test_s3_config(None), "pdf"), "pdf/");
        assert_eq!(folder_prefix(&test_s3_config(Some("pdf")), ""), "pdf/");
        assert_eq!(
            folder_prefix(&test_s3_config(Some("pdf/")), "中文/嵌套"),
            "pdf/中文/嵌套/"
        );
        assert_eq!(
            folder_object_key(&test_s3_config(Some("pdf/")), "中文/嵌套"),
            "pdf/中文/嵌套/"
        );
    }

    #[test]
    fn rust_s3_list_dto_matches_frontend_sample() {
        let result = S3ListResult {
            items: vec![
                S3FileItem {
                    key: "pdf/中文/报告.pdf".into(),
                    name: "报告.pdf".into(),
                    size: 42,
                    last_modified: "2026-09-27T12:00:00Z".into(),
                    is_dir: false,
                },
                S3FileItem {
                    key: "pdf/中文/嵌套/".into(),
                    name: "嵌套".into(),
                    size: 0,
                    last_modified: String::new(),
                    is_dir: true,
                },
            ],
            prefixes: Vec::new(),
            common_prefixes: vec!["pdf/中文/嵌套/".into()],
        };
        let sample: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/remediation-05/s3-list-result.json"
        ))
        .unwrap();
        assert_eq!(serde_json::to_value(result).unwrap(), sample);
    }

    #[test]
    fn rust_s3_version_dto_uses_frontend_field_names() {
        let result = S3VersionsResult {
            versions: vec![S3VersionItem {
                version_id: "v1".into(),
                size: 7,
                last_modified: "2026-09-27T12:00:00Z".into(),
                is_latest: true,
            }],
            delete_markers: vec!["d1".into()],
        };
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["versions"][0]["version_id"], "v1");
        assert_eq!(
            value["versions"][0]["last_modified"],
            "2026-09-27T12:00:00Z"
        );
        assert_eq!(value["versions"][0]["is_latest"], true);
        assert_eq!(value["delete_markers"][0], "d1");
    }

    #[test]
    fn test_sync_backup_info_serde_roundtrip() {
        let info = SyncBackupInfo {
            backed_at: "1700000000".into(),
            app_version: "0.4.0".into(),
            checksum: "abc123".into(),
        };
        let json = serde_json::to_vec(&info).unwrap();
        let parsed: SyncBackupInfo = serde_json::from_slice(&json).unwrap();
        assert_eq!(parsed.backed_at, "1700000000");
        assert_eq!(parsed.app_version, "0.4.0");
        assert_eq!(parsed.checksum, "abc123");
    }

    #[test]
    fn test_config_toml_roundtrip_with_sync_fields() {
        let cfg = AppConfig {
            general: GeneralConfig {
                language: "zh".into(),
                theme: "dark".into(),
                default_export_dir: None,
                recent_files_max: 10,
                recent_files: vec!["/tmp/a.pdf".into()],
                auto_update_check: true,
            },
            s3: Some(S3Config {
                auto_backup_config: true,
                last_backup_at: Some("1700000000".into()),
                ..test_s3_config(Some("pdf"))
            }),
            recovery_notice: None,
        };
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        // Old configs without the new fields must still parse (serde defaults)
        let legacy = toml_str
            .replace("\nauto_backup_config = true", "")
            .replace("\nlast_backup_at = \"1700000000\"", "");
        let parsed: AppConfig = toml::from_str(&legacy).unwrap();
        let s3 = parsed.s3.unwrap();
        assert!(!s3.auto_backup_config);
        assert!(s3.last_backup_at.is_none());
        // Full roundtrip preserves everything
        let parsed: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.general.recent_files, vec!["/tmp/a.pdf".to_string()]);
        assert_eq!(
            parsed.s3.unwrap().last_backup_at.as_deref(),
            Some("1700000000")
        );
    }

    fn backup_test_config() -> AppConfig {
        AppConfig {
            general: GeneralConfig {
                language: "en".into(),
                theme: "system".into(),
                default_export_dir: Some("/Users/test/export".into()),
                recent_files_max: 10,
                recent_files: vec!["/Users/test/private/report.pdf".into()],
                auto_update_check: false,
            },
            s3: Some(S3Config {
                access_key: "TEST_ACCESS_KEY".into(),
                secret_key: "TEST_SECRET_KEY".into(),
                session_token: Some("TEST_SESSION_TOKEN".into()),
                ..test_s3_config(Some("pdf"))
            }),
            recovery_notice: Some("/Users/test/config.toml.corrupt".into()),
        }
    }

    #[test]
    fn config_backup_excludes_credentials_and_recent_paths_and_checksums_payload() {
        let config = backup_test_config();
        let (bytes, info) =
            make_config_backup(&config, "1700000000".into(), "0.4.0".into()).unwrap();
        let serialized = String::from_utf8(bytes.clone()).unwrap();
        assert!(!serialized.contains("TEST_ACCESS_KEY"));
        assert!(!serialized.contains("TEST_SECRET_KEY"));
        assert!(!serialized.contains("TEST_SESSION_TOKEN"));
        assert!(!serialized.contains("/Users/test/private/report.pdf"));
        assert!(!serialized.contains("/Users/test/config.toml.corrupt"));
        let backup: toml::Value = toml::from_str(&serialized).unwrap();
        assert_eq!(
            backup["backup"]["format_version"].as_integer(),
            Some(CONFIG_BACKUP_FORMAT_VERSION as i64)
        );
        assert_eq!(
            backup["backup"]["checksum"].as_str(),
            Some(info.checksum.as_str())
        );

        let restored = parse_config_backup(&bytes).unwrap();
        assert!(restored.general.recent_files.is_empty());
        let s3 = restored.s3.unwrap();
        assert!(s3.access_key.is_empty());
        assert!(s3.secret_key.is_empty());
        assert!(s3.session_token.is_none());
    }

    #[test]
    fn config_backup_rejects_a_modified_payload() {
        let (bytes, _) =
            make_config_backup(&backup_test_config(), "1700000000".into(), "0.4.0".into()).unwrap();
        let mut backup: toml::Value = toml::from_str(std::str::from_utf8(&bytes).unwrap()).unwrap();
        backup["general"]["language"] = toml::Value::String("zh".into());
        let tampered = toml::to_string_pretty(&backup).unwrap();
        assert!(parse_config_backup(tampered.as_bytes()).is_err());
    }

    #[test]
    fn new_backup_remains_readable_by_legacy_app_config_deserializer() {
        let (bytes, _) =
            make_config_backup(&backup_test_config(), "1700000000".into(), "0.4.0".into()).unwrap();
        let legacy_reader: AppConfig =
            toml::from_str(std::str::from_utf8(&bytes).unwrap()).unwrap();
        assert_eq!(legacy_reader.general.language, "en");
        assert_eq!(legacy_reader.s3.as_ref().unwrap().bucket, "test-bucket");
        assert!(legacy_reader.s3.unwrap().access_key.is_empty());
    }

    #[test]
    fn legacy_toml_backup_is_read_but_local_only_data_is_stripped() {
        let legacy = toml::to_string_pretty(&backup_test_config()).unwrap();
        let restored = parse_config_backup(legacy.as_bytes()).unwrap();
        assert!(restored.general.recent_files.is_empty());
        assert!(restored.s3.as_ref().unwrap().access_key.is_empty());
    }

    #[test]
    fn restoring_matching_connection_preserves_local_credentials_and_recent_paths() {
        let local = backup_test_config();
        let mut restored = backup_safe_config(&local);

        preserve_local_only_data(&mut restored, &local);

        assert_eq!(
            restored.general.recent_files,
            vec!["/Users/test/private/report.pdf"]
        );
        let s3 = restored.s3.unwrap();
        assert_eq!(s3.access_key, "TEST_ACCESS_KEY");
        assert_eq!(s3.secret_key, "TEST_SECRET_KEY");
        assert_eq!(s3.session_token.as_deref(), Some("TEST_SESSION_TOKEN"));
    }
}
