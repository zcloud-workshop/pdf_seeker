use aws_sdk_s3::primitives::ByteStream;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::config::S3Config;
use crate::error::AppError;
use crate::error::AppResult;

async fn build_client(cfg: &S3Config) -> AppResult<aws_sdk_s3::Client> {
    let endpoint = cfg.endpoint.clone();
    let region = cfg.region.clone();
    let force_path_style = cfg.force_path_style;

    match cfg.auth_mode {
        crate::config::S3AuthMode::None => {
            // Public bucket — build client directly, skip default credential chain entirely
            let mut builder = aws_sdk_s3::config::Builder::new()
                .behavior_version(aws_config::BehaviorVersion::latest())
                .region(aws_sdk_s3::config::Region::new(if region.is_empty() { "us-east-1".into() } else { region }))
                .force_path_style(force_path_style)
                .credentials_provider(
                    aws_sdk_s3::config::SharedCredentialsProvider::new(
                        aws_sdk_s3::config::Credentials::new("ANONYMOUS", "", None, None, "anon"),
                    ),
                );

            if !endpoint.is_empty() {
                builder = builder.endpoint_url(endpoint);
            }

            Ok(aws_sdk_s3::Client::from_conf(builder.build()))
        }
        _ => {
            // Static or Env mode — use default config loader
            let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest());

            if !endpoint.is_empty() && !endpoint.contains("amazonaws.com") {
                config_loader = config_loader.endpoint_url(&endpoint);
            }

            let sdk_config = config_loader.load().await;

            let mut builder = aws_sdk_s3::config::Builder::from(&sdk_config)
                .region(aws_sdk_s3::config::Region::new(if region.is_empty() { "us-east-1".into() } else { region }))
                .force_path_style(force_path_style);

            if matches!(cfg.auth_mode, crate::config::S3AuthMode::Static) {
                let access_key = cfg.access_key.clone();
                let secret_key = cfg.secret_key.clone();
                let session_token = cfg.session_token.clone();
                builder = builder.credentials_provider(
                    aws_sdk_s3::config::SharedCredentialsProvider::new(
                        aws_sdk_s3::config::Credentials::new(
                            &access_key,
                            &secret_key,
                            session_token,
                            None,
                            "pdf-seeker",
                        ),
                    ),
                );
            }
            // Env mode: sdk_config already picks up env vars / instance profile

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
    let base = prefix(&s3_config);
    let full_prefix = if folder.is_empty() {
        format!("{}/", base)
    } else if folder.starts_with('/') {
        format!("{}{}/", base, &folder[1..])
    } else {
        format!("{}/{}/", base, folder)
    };

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
                if key.ends_with('/') && key == full_prefix {
                    continue;
                }
                let name = key
                    .trim_start_matches(&full_prefix)
                    .to_string();
                let name = name.trim_end_matches('/').to_string();
                if name.is_empty() {
                    continue;
                }
                let size = obj.size.unwrap_or(0) as u64;
                let last_modified = obj
                    .last_modified
                    .map(|t| t.to_string())
                    .unwrap_or_default();
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
    remote_key: String,
) -> AppResult<()> {
    let client = build_client(&s3_config).await?;
    let base = prefix(&s3_config);
    let key = if remote_key.starts_with('/') {
        format!("{}{}", base, &remote_key[1..])
    } else {
        format!("{}/{}", base, remote_key)
    };

    let file_name = Path::new(&local_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "upload".to_string());
    let final_key = if key.ends_with('/') || key.is_empty() {
        format!("{}{}", key, file_name)
    } else {
        key
    };

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
    let base = prefix(&s3_config);
    let key = if remote_key.starts_with('/') {
        format!("{}{}", base, &remote_key[1..])
    } else {
        format!("{}/{}", base, remote_key)
    };

    let resp = client
        .get_object()
        .bucket(&s3_config.bucket)
        .key(&key)
        .send()
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;

    let data = resp.body.collect().await.map_err(|e| AppError::S3(e.to_string()))?;
    let bytes = data.to_vec();

    // Ensure parent directory exists
    if let Some(parent) = Path::new(&local_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&local_path, bytes)?;

    Ok(())
}

#[tauri::command]
pub async fn s3_delete_file(s3_config: S3Config, remote_key: String) -> AppResult<()> {
    let client = build_client(&s3_config).await?;
    let base = prefix(&s3_config);
    let key = if remote_key.starts_with('/') {
        format!("{}{}", base, &remote_key[1..])
    } else {
        format!("{}/{}", base, remote_key)
    };

    client
        .delete_object()
        .bucket(&s3_config.bucket)
        .key(&key)
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
    let base = prefix(&s3_config);
    let key = if remote_key.starts_with('/') {
        format!("{}{}", base, &remote_key[1..])
    } else {
        format!("{}/{}", base, remote_key)
    };

    let max_versions = s3_config.max_versions.unwrap_or(20) as i32;

    let resp = client
        .list_object_versions()
        .bucket(&s3_config.bucket)
        .prefix(&key)
        .max_keys(max_versions)
        .send()
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;

    let mut versions = Vec::new();
    let mut delete_markers = Vec::new();

    if let Some(vers) = resp.versions {
        for v in vers {
            let vid = v.version_id.unwrap_or_else(|| "null".to_string());
            let size = v.size.unwrap_or(0) as u64;
            let last_modified = v
                .last_modified
                .map(|t| t.to_string())
                .unwrap_or_default();
            let is_latest = v.is_latest.unwrap_or(false);
            versions.push(S3VersionItem {
                version_id: vid,
                size,
                last_modified,
                is_latest,
            });
        }
    }

    if let Some(dm) = resp.delete_markers {
        for d in dm {
            delete_markers.push(d.version_id.unwrap_or_else(|| "null".to_string()));
        }
    }

    Ok(S3VersionsResult {
        versions,
        delete_markers,
    })
}

#[tauri::command]
pub async fn s3_delete_version(
    s3_config: S3Config,
    remote_key: String,
    version_id: String,
) -> AppResult<()> {
    let client = build_client(&s3_config).await?;
    let base = prefix(&s3_config);
    let key = if remote_key.starts_with('/') {
        format!("{}{}", base, &remote_key[1..])
    } else {
        format!("{}/{}", base, remote_key)
    };

    client
        .delete_object()
        .bucket(&s3_config.bucket)
        .key(&key)
        .version_id(&version_id)
        .send()
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub async fn s3_create_folder(s3_config: S3Config, folder_name: String) -> AppResult<()> {
    let client = build_client(&s3_config).await?;
    let base = prefix(&s3_config);
    let key = format!("{}/{}/", base, folder_name.trim_matches('/'));

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
    let base = prefix(&s3_config);
    let key = if remote_key.starts_with('/') {
        format!("{}{}", base, &remote_key[1..])
    } else {
        format!("{}/{}", base, remote_key)
    };

    let presigned = client
        .get_object()
        .bucket(&s3_config.bucket)
        .key(&key)
        .presigned(
            aws_sdk_s3::presigning::PresigningConfig::builder()
                .expires_in(std::time::Duration::from_secs(expires_in_secs))
                .build()?,
        )
        .await
        .map_err(|e| AppError::S3(e.to_string()))?;

    Ok(presigned.uri().to_string())
}
