use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::Manager;

mod process;
use process::run_process;

const DEFAULT_TIMEOUT_SECS: u64 = 120;
static OUTPUT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// A plugin manifest (`{app_config_dir}/plugins/<id>/plugin.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Executable name; relative paths resolve inside the plugin directory,
    /// bare names resolve via PATH
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub output: PluginOutput,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginOutput {
    /// "text" (plugin prints to stdout) | "file" (plugin writes to {output})
    pub mode: String,
    #[serde(default)]
    pub extension: Option<String>,
}

impl Default for PluginOutput {
    fn default() -> Self {
        Self {
            mode: "text".to_string(),
            extension: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPluginRequest {
    pub plugin_id: String,
    #[serde(default)]
    pub input_path: Option<String>,
    #[serde(default)]
    pub output_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRunResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

fn plugins_dir(handle: &tauri::AppHandle) -> AppResult<PathBuf> {
    Ok(handle
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Config(e.to_string()))?
        .join("plugins"))
}

fn validate_plugin_id(plugin_id: &str) -> AppResult<()> {
    let bytes = plugin_id.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && plugin_id != "."
        && plugin_id != ".."
        && !plugin_id.ends_with('.');
    if !valid {
        return Err(AppError::Config(format!(
            "Invalid plugin directory ID '{}'",
            plugin_id
        )));
    }
    Ok(())
}

fn canonical_plugin_dir(root: &Path, plugin_id: &str) -> AppResult<PathBuf> {
    validate_plugin_id(plugin_id)?;
    let canonical_root = root
        .canonicalize()
        .map_err(|e| AppError::Config(format!("Cannot resolve plugin directory: {}", e)))?;
    let candidate = canonical_root.join(plugin_id);
    let metadata = std::fs::symlink_metadata(&candidate).map_err(|e| {
        AppError::Config(format!(
            "Cannot inspect plugin directory '{}': {}",
            candidate.display(),
            e
        ))
    })?;
    if !metadata.file_type().is_dir() {
        return Err(AppError::Config(format!(
            "Plugin directory '{}' must be a real directory",
            plugin_id
        )));
    }
    let canonical_dir = candidate.canonicalize().map_err(|e| {
        AppError::Config(format!(
            "Cannot resolve plugin directory '{}': {}",
            plugin_id, e
        ))
    })?;
    if !canonical_dir.starts_with(&canonical_root) {
        return Err(AppError::Config(format!(
            "Plugin directory '{}' resolves outside the plugins directory",
            plugin_id
        )));
    }
    Ok(canonical_dir)
}

fn read_manifest_in(plugin_dir: &Path, plugin_id: &str) -> AppResult<PluginManifest> {
    let manifest_path = plugin_dir.join("plugin.json");
    let canonical_manifest = manifest_path.canonicalize().map_err(|e| {
        AppError::Config(format!(
            "Cannot read plugin manifest '{}': {}",
            manifest_path.display(),
            e
        ))
    })?;
    if !canonical_manifest.starts_with(plugin_dir) {
        return Err(AppError::Config(format!(
            "Plugin manifest for '{}' resolves outside its plugin directory",
            plugin_id
        )));
    }
    let content = std::fs::read_to_string(&canonical_manifest).map_err(|e| {
        AppError::Config(format!(
            "Cannot read plugin manifest '{}': {}",
            canonical_manifest.display(),
            e
        ))
    })?;
    let mut manifest: PluginManifest = serde_json::from_str(&content)
        .map_err(|e| AppError::Config(format!("Invalid plugin manifest '{}': {}", plugin_id, e)))?;
    if manifest.id.is_empty() {
        manifest.id = plugin_id.to_string();
    } else if manifest.id != plugin_id {
        return Err(AppError::Config(format!(
            "Plugin manifest ID '{}' does not match directory '{}'",
            manifest.id, plugin_id
        )));
    }
    if manifest.name.trim().is_empty() {
        return Err(AppError::Config(format!(
            "Plugin '{}' manifest has an empty name",
            plugin_id
        )));
    }
    if manifest.command.trim().is_empty() {
        return Err(AppError::Config(format!(
            "Plugin '{}' manifest has an empty command",
            plugin_id
        )));
    }
    if !matches!(manifest.output.mode.as_str(), "text" | "file") {
        return Err(AppError::Config(format!(
            "Plugin '{}' has unsupported output mode '{}'",
            plugin_id, manifest.output.mode
        )));
    }
    if manifest.output.mode == "file" {
        if let Some(extension) = manifest.output.extension.as_deref() {
            validate_extension(extension)?;
        }
    }
    Ok(manifest)
}

fn read_manifest(handle: &tauri::AppHandle, plugin_id: &str) -> AppResult<PluginManifest> {
    let root = plugins_dir(handle)?;
    let plugin_dir = canonical_plugin_dir(&root, plugin_id)?;
    read_manifest_in(&plugin_dir, plugin_id)
}

#[tauri::command]
pub fn list_plugins(app: tauri::AppHandle) -> AppResult<Vec<PluginManifest>> {
    let root = plugins_dir(&app)?;
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(_) => return Ok(Vec::new()),
    };
    let canonical_root = match root.canonicalize() {
        Ok(root) => root,
        Err(_) => return Ok(Vec::new()),
    };
    let mut plugins = Vec::new();
    for entry in entries.flatten() {
        let Some(plugin_id) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if validate_plugin_id(&plugin_id).is_err() {
            continue;
        }
        let metadata = match std::fs::symlink_metadata(entry.path()) {
            Ok(metadata) if metadata.file_type().is_dir() => metadata,
            _ => continue,
        };
        drop(metadata);
        let plugin_dir = match entry.path().canonicalize() {
            Ok(path) if path.starts_with(&canonical_root) => path,
            _ => continue,
        };
        match read_manifest_in(&plugin_dir, &plugin_id) {
            Ok(manifest) => plugins.push(manifest),
            Err(error) => tracing::warn!(
                "Skipping invalid plugin manifest in {}: {}",
                plugin_dir.display(),
                error
            ),
        }
    }
    plugins.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(plugins)
}

/// Replace `{input}` / `{output}` / `{dir}` placeholders. Args are passed to
/// the child process directly (no shell), so placeholders are the only
/// substitution mechanism — no injection surface.
pub(crate) fn substitute_arg(
    arg: &str,
    plugin_dir: &Path,
    input_path: Option<&str>,
    output_path: Option<&str>,
) -> AppResult<String> {
    let mut out = arg.to_string();
    if out.contains("{input}") {
        let input = input_path.ok_or_else(|| {
            AppError::Config("Plugin expects {input} but no input file was provided".into())
        })?;
        out = out.replace("{input}", input);
    }
    if out.contains("{output}") {
        let output = output_path.ok_or_else(|| {
            AppError::Config("Plugin expects {output} but no output path was chosen".into())
        })?;
        out = out.replace("{output}", output);
    }
    Ok(out.replace("{dir}", &plugin_dir.to_string_lossy()))
}

/// Resolve relative executable paths inside the plugin directory. Bare names
/// not found there retain the existing PATH lookup behavior.
pub(crate) fn resolve_command(plugin_dir: &Path, command: &str) -> AppResult<String> {
    let candidate = Path::new(command);
    if candidate.is_absolute() {
        return Ok(command.to_string());
    }
    if candidate.components().any(|part| {
        matches!(
            part,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(AppError::Config(
            "Relative plugin commands cannot leave the plugin directory".into(),
        ));
    }

    let joined = plugin_dir.join(candidate);
    match std::fs::symlink_metadata(&joined) {
        Ok(_) => {
            let canonical = joined
                .canonicalize()
                .map_err(|e| AppError::Config(format!("Cannot resolve plugin command: {}", e)))?;
            if !canonical.starts_with(plugin_dir) {
                return Err(AppError::Config(
                    "Plugin command resolves outside the plugin directory".into(),
                ));
            }
            Ok(canonical.to_string_lossy().to_string())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let has_path =
                command.contains('/') || command.contains('\\') || command.starts_with('.');
            if has_path {
                let parent = joined
                    .parent()
                    .unwrap_or(plugin_dir)
                    .canonicalize()
                    .map_err(|e| {
                        AppError::Config(format!("Cannot resolve plugin command directory: {}", e))
                    })?;
                if !parent.starts_with(plugin_dir) {
                    return Err(AppError::Config(
                        "Plugin command resolves outside the plugin directory".into(),
                    ));
                }
                Ok(joined.to_string_lossy().to_string())
            } else {
                Ok(command.to_string())
            }
        }
        Err(error) => Err(AppError::Config(format!(
            "Cannot inspect plugin command: {}",
            error
        ))),
    }
}

fn validate_extension(extension: &str) -> AppResult<()> {
    if extension.is_empty()
        || extension.len() > 16
        || !extension
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(AppError::Config(format!(
            "Invalid plugin output extension '{}'",
            extension
        )));
    }
    Ok(())
}

fn output_paths(requested_path: &str, extension: Option<&str>) -> AppResult<(PathBuf, PathBuf)> {
    let requested = Path::new(requested_path);
    let file_name = requested
        .file_name()
        .ok_or_else(|| AppError::Config("Plugin output path must name a file".into()))?;
    let parent = requested
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = parent.canonicalize().map_err(|e| {
        AppError::Config(format!(
            "Cannot resolve plugin output directory '{}': {}",
            parent.display(),
            e
        ))
    })?;
    if !parent.is_dir() {
        return Err(AppError::Config(format!(
            "Plugin output directory '{}' is not a directory",
            parent.display()
        )));
    }
    let target = parent.join(file_name);
    match std::fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => {
            return Err(AppError::Config(format!(
                "Plugin output path '{}' is not a regular file",
                target.display()
            )))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(AppError::Config(format!(
                "Cannot inspect plugin output path '{}': {}",
                target.display(),
                error
            )))
        }
    }
    if let Some(expected) = extension {
        let actual = target
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(AppError::Config(format!(
                "Plugin output '{}' must use the declared .{} extension",
                target.display(),
                expected
            )));
        }
    }

    let stem = target.file_stem().unwrap_or(file_name).to_string_lossy();
    let extension = target.extension().and_then(|value| value.to_str());
    let temporary = loop {
        let sequence = OUTPUT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = match extension {
            Some(extension) => format!(
                ".{}.pdf-seeker-{}-{}.{}",
                stem,
                std::process::id(),
                sequence,
                extension
            ),
            None => format!(".{}.pdf-seeker-{}-{}", stem, std::process::id(), sequence),
        };
        let candidate = parent.join(name);
        if std::fs::symlink_metadata(&candidate).is_err() {
            break candidate;
        }
    };
    Ok((target, temporary))
}

fn publish_output(temporary: &Path, target: &Path) -> AppResult<()> {
    let output_metadata = std::fs::symlink_metadata(temporary).map_err(|e| {
        AppError::Config(format!(
            "Plugin did not create output file '{}': {}",
            target.display(),
            e
        ))
    })?;
    if !output_metadata.file_type().is_file() {
        return Err(AppError::Config(format!(
            "Plugin output '{}' is not a regular file",
            target.display()
        )));
    }
    match std::fs::symlink_metadata(target) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => {
            return Err(AppError::Config(format!(
                "Plugin output path '{}' is not a regular file",
                target.display()
            )))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(AppError::Config(format!(
                "Cannot inspect plugin output path '{}': {}",
                target.display(),
                error
            )))
        }
    }

    if !target.exists() {
        return std::fs::rename(temporary, target).map_err(|e| {
            AppError::Config(format!(
                "Cannot publish plugin output '{}': {}",
                target.display(),
                e
            ))
        });
    }

    let parent = target.parent().unwrap_or(Path::new("."));
    let file_name = target.file_name().unwrap_or_default().to_string_lossy();
    let backup = loop {
        let sequence = OUTPUT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{}.pdf-seeker-backup-{}-{}",
            file_name,
            std::process::id(),
            sequence
        ));
        if std::fs::symlink_metadata(&candidate).is_err() {
            break candidate;
        }
    };
    std::fs::rename(target, &backup).map_err(|e| {
        AppError::Config(format!(
            "Cannot replace plugin output '{}': {}",
            target.display(),
            e
        ))
    })?;
    if let Err(error) = std::fs::rename(temporary, target) {
        if let Err(restore_error) = std::fs::rename(&backup, target) {
            return Err(AppError::Config(format!(
                "Cannot publish plugin output '{}': {}; previous output remains at '{}': {}",
                target.display(),
                error,
                backup.display(),
                restore_error
            )));
        }
        return Err(AppError::Config(format!(
            "Cannot publish plugin output '{}': {}",
            target.display(),
            error
        )));
    }
    if let Err(error) = std::fs::remove_file(&backup) {
        tracing::warn!(
            "Could not remove previous plugin output {}: {}",
            backup.display(),
            error
        );
    }
    Ok(())
}

fn run_file_plugin(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    target: &Path,
    temporary: &Path,
) -> AppResult<PluginRunResult> {
    let result = match run_process(program, args, cwd, timeout) {
        Ok(result) => result,
        Err(error) => {
            let _ = std::fs::remove_file(temporary);
            return Err(error);
        }
    };
    if result.timed_out {
        let _ = std::fs::remove_file(temporary);
        return Err(AppError::Config(format!(
            "Plugin timed out before completing output '{}': process tree was stopped",
            target.display()
        )));
    }
    if result.exit_code != Some(0) {
        let _ = std::fs::remove_file(temporary);
        return Err(AppError::Config(format!(
            "Plugin exited with code {:?} before completing output '{}'",
            result.exit_code,
            target.display()
        )));
    }
    if let Err(error) = publish_output(temporary, target) {
        let _ = std::fs::remove_file(temporary);
        return Err(error);
    }
    Ok(result)
}

#[tauri::command]
pub async fn run_plugin(
    app: tauri::AppHandle,
    req: RunPluginRequest,
) -> AppResult<PluginRunResult> {
    let manifest = read_manifest(&app, &req.plugin_id)?;
    let plugin_dir = canonical_plugin_dir(&plugins_dir(&app)?, &req.plugin_id)?;
    let program = resolve_command(&plugin_dir, &manifest.command)?;
    let is_file_mode = manifest.output.mode == "file";
    let (output_target, output_temporary) = if is_file_mode {
        if !manifest.args.iter().any(|arg| arg.contains("{output}")) {
            return Err(AppError::Config(
                "File-output plugins must pass the {output} placeholder".into(),
            ));
        }
        let requested = req
            .output_path
            .as_deref()
            .filter(|path| !path.is_empty())
            .ok_or_else(|| {
                AppError::Config(
                    "This plugin writes an output file; choose an output path first".into(),
                )
            })?;
        let (target, temporary) = output_paths(requested, manifest.output.extension.as_deref())?;
        (Some(target), Some(temporary))
    } else {
        (None, None)
    };
    let output_argument = output_temporary
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    let args: Vec<String> = manifest
        .args
        .iter()
        .map(|arg| {
            substitute_arg(
                arg,
                &plugin_dir,
                req.input_path.as_deref(),
                output_argument.as_deref(),
            )
        })
        .collect::<AppResult<Vec<_>>>()?;
    let timeout = Duration::from_secs(manifest.timeout_secs.max(1));

    tauri::async_runtime::spawn_blocking(move || {
        if let (Some(target), Some(temporary)) = (output_target, output_temporary) {
            run_file_plugin(&program, &args, &plugin_dir, timeout, &target, &temporary)
        } else {
            run_process(&program, &args, &plugin_dir, timeout)
        }
    })
    .await
    .map_err(|e| AppError::Config(format!("Plugin task failed: {}", e)))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_parse_camel_case_with_defaults() {
        let json = r#"{
            "name": "Page Count",
            "command": "./count.sh",
            "args": ["{input}"]
        }"#;
        let m: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.name, "Page Count");
        assert_eq!(m.command, "./count.sh");
        assert_eq!(m.args, vec!["{input}"]);
        assert_eq!(m.output.mode, "text");
        assert_eq!(m.timeout_secs, DEFAULT_TIMEOUT_SECS);
        assert!(m.version.is_none());
    }

    #[test]
    fn test_manifest_parse_full() {
        let json = r#"{
            "id": "my-tool",
            "name": "My Tool",
            "version": "1.0.0",
            "description": "desc",
            "command": "python3",
            "args": ["{dir}/run.py", "{input}", "{output}"],
            "output": { "mode": "file", "extension": "txt" },
            "timeoutSecs": 30
        }"#;
        let m: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.id, "my-tool");
        assert_eq!(m.output.mode, "file");
        assert_eq!(m.output.extension.as_deref(), Some("txt"));
        assert_eq!(m.timeout_secs, 30);
    }

    #[test]
    fn test_plugin_ids_reject_path_components() {
        assert!(validate_plugin_id("file-info").is_ok());
        assert!(validate_plugin_id("../outside").is_err());
        assert!(validate_plugin_id("/outside").is_err());
        assert!(validate_plugin_id("bad\\id").is_err());
    }

    #[test]
    fn test_manifest_id_is_directory_identity() {
        let temp = tempfile::tempdir().unwrap();
        let plugin_dir = temp.path().join("right-id");
        std::fs::create_dir(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"id":"wrong-id","name":"Example","command":"tool"}"#,
        )
        .unwrap();
        assert!(read_manifest_in(&plugin_dir, "right-id").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_plugin_directory_symlink_is_rejected() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), root.path().join("linked")).unwrap();
        assert!(canonical_plugin_dir(root.path(), "linked").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_manifest_and_command_symlinks_cannot_escape_plugin_directory() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let plugin_dir = root.path().join("example");
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir(&plugin_dir).unwrap();
        std::fs::write(
            outside.path().join("plugin.json"),
            r#"{"id":"example","name":"Example","command":"run"}"#,
        )
        .unwrap();
        std::fs::write(outside.path().join("run"), "not executable").unwrap();
        symlink(
            outside.path().join("plugin.json"),
            plugin_dir.join("plugin.json"),
        )
        .unwrap();
        symlink(outside.path().join("run"), plugin_dir.join("run")).unwrap();

        let canonical_dir = plugin_dir.canonicalize().unwrap();
        assert!(read_manifest_in(&canonical_dir, "example").is_err());
        assert!(resolve_command(&canonical_dir, "./run").is_err());
    }

    #[test]
    fn test_substitute_arg() {
        let dir = Path::new("/plugins/demo");
        assert_eq!(
            substitute_arg("{dir}/run.py {input}", dir, Some("/tmp/a.pdf"), None).unwrap(),
            "/plugins/demo/run.py /tmp/a.pdf"
        );
        assert_eq!(
            substitute_arg("{output}", dir, None, Some("/tmp/out.txt")).unwrap(),
            "/tmp/out.txt"
        );
        assert!(substitute_arg("{input}", dir, None, None).is_err());
        assert!(substitute_arg("{output}", dir, None, None).is_err());
    }

    #[test]
    fn test_resolve_command() {
        let dir = std::env::temp_dir();
        assert_eq!(resolve_command(&dir, "python3").unwrap(), "python3");
        assert_eq!(
            resolve_command(&dir, "/usr/local/bin/tool").unwrap(),
            "/usr/local/bin/tool"
        );
        assert!(resolve_command(&dir, "../outside/tool").is_err());
    }

    #[test]
    fn test_output_paths_enforce_declared_extension() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            output_paths(dir.path().join("result.pdf").to_str().unwrap(), Some("txt")).is_err()
        );
        assert!(output_paths(dir.path().join("result.txt").to_str().unwrap(), Some("txt")).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn test_file_output_is_published_only_after_success() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("result.txt");
        let (actual_target, temporary) =
            output_paths(target.to_str().unwrap(), Some("txt")).unwrap();
        let args = vec![
            "-c".to_string(),
            "printf 'fresh' > \"$1\"".to_string(),
            "plugin".to_string(),
            temporary.to_string_lossy().into_owned(),
        ];
        let result = run_file_plugin(
            "/bin/sh",
            &args,
            dir.path(),
            Duration::from_secs(5),
            &actual_target,
            &temporary,
        )
        .unwrap();
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "fresh");

        std::fs::write(&target, "old").unwrap();
        let (actual_target, temporary) =
            output_paths(target.to_str().unwrap(), Some("txt")).unwrap();
        let result = run_file_plugin(
            "/bin/sh",
            &["-c".into(), "exit 0".into()],
            dir.path(),
            Duration::from_secs(5),
            &actual_target,
            &temporary,
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "old");
    }
}
