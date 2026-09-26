use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::Manager;

const DEFAULT_TIMEOUT_SECS: u64 = 120;
const MAX_OUTPUT_BYTES: usize = 200 * 1024;

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

fn read_manifest(handle: &tauri::AppHandle, plugin_id: &str) -> AppResult<PluginManifest> {
    let dir = plugins_dir(handle)?;
    let manifest_path = dir.join(plugin_id).join("plugin.json");
    let content = std::fs::read_to_string(&manifest_path).map_err(|e| {
        AppError::Config(format!(
            "Cannot read plugin manifest '{}': {}",
            manifest_path.display(),
            e
        ))
    })?;
    let manifest: PluginManifest =
        serde_json::from_str(&content).map_err(|e| {
            AppError::Config(format!("Invalid plugin manifest '{}': {}", plugin_id, e))
        })?;
    if manifest.name.trim().is_empty() {
        return Err(AppError::Config(format!(
            "Plugin '{}' manifest has an empty name",
            plugin_id
        )));
    }
    Ok(manifest)
}

#[tauri::command]
pub fn list_plugins(app: tauri::AppHandle) -> AppResult<Vec<PluginManifest>> {
    let dir = plugins_dir(&app)?;
    let mut plugins = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(plugins), // no plugins dir yet
    };
    for entry in entries.flatten() {
        let manifest_path = entry.path().join("plugin.json");
        if !manifest_path.is_file() {
            continue;
        }
        match std::fs::read_to_string(&manifest_path)
            .map_err(AppError::from)
            .and_then(|s| serde_json::from_str::<PluginManifest>(&s).map_err(AppError::from))
        {
            Ok(mut m) => {
                if m.id.trim().is_empty() {
                    m.id = entry.file_name().to_string_lossy().to_string();
                }
                plugins.push(m);
            }
            Err(e) => tracing::warn!(
                "Skipping invalid plugin manifest {}: {}",
                manifest_path.display(),
                e
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
            AppError::Config(
                "Plugin expects {output} but no output path was chosen".into(),
            )
        })?;
        out = out.replace("{output}", output);
    }
    Ok(out.replace("{dir}", &plugin_dir.to_string_lossy()))
}

/// Resolve the command: absolute paths pass through, relative/`./` paths
/// resolve inside the plugin directory, bare names fall back to PATH lookup.
pub(crate) fn resolve_command(plugin_dir: &Path, command: &str) -> String {
    let candidate = Path::new(command);
    if candidate.is_absolute() {
        return command.to_string();
    }
    let joined = plugin_dir.join(candidate);
    if joined.exists() {
        return joined.to_string_lossy().to_string();
    }
    command.to_string()
}

fn truncate_output(s: &str) -> String {
    if s.len() > MAX_OUTPUT_BYTES {
        let mut cut = s[..MAX_OUTPUT_BYTES].to_string();
        cut.push_str("\n...[output truncated]");
        cut
    } else {
        s.to_string()
    }
}

pub(crate) fn run_process(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
) -> AppResult<PluginRunResult> {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| {
        AppError::Config(format!("Failed to start plugin command '{}': {}", program, e))
    })?;

    // Drain pipes on dedicated threads so a chatty plugin can't deadlock
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let out_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut s) = stdout_pipe {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let err_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut s) = stderr_pipe {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(AppError::Config(format!("Plugin process error: {}", e)));
            }
        }
    };

    let stdout = String::from_utf8_lossy(&out_handle.join().unwrap_or_default()).to_string();
    let stderr = String::from_utf8_lossy(&err_handle.join().unwrap_or_default()).to_string();

    Ok(PluginRunResult {
        exit_code: status.and_then(|s| s.code()),
        stdout: truncate_output(&stdout),
        stderr: truncate_output(&stderr),
        timed_out,
    })
}

#[tauri::command]
pub async fn run_plugin(
    app: tauri::AppHandle,
    req: RunPluginRequest,
) -> AppResult<PluginRunResult> {
    let manifest = read_manifest(&app, &req.plugin_id)?;
    if manifest.command.trim().is_empty() {
        return Err(AppError::Config(format!(
            "Plugin '{}' has an empty command",
            req.plugin_id
        )));
    }
    let is_file_mode = manifest.output.mode == "file";
    if is_file_mode && req.output_path.as_deref().unwrap_or("").is_empty() {
        return Err(AppError::Config(
            "This plugin writes an output file; choose an output path first".into(),
        ));
    }

    let plugin_dir = plugins_dir(&app)?.join(&req.plugin_id);
    let program = resolve_command(&plugin_dir, &manifest.command);
    let args: Vec<String> = manifest
        .args
        .iter()
        .map(|a| substitute_arg(a, &plugin_dir, req.input_path.as_deref(), req.output_path.as_deref()))
        .collect::<AppResult<Vec<_>>>()?;
    let timeout = Duration::from_secs(manifest.timeout_secs.max(1));

    tauri::async_runtime::spawn_blocking(move || run_process(&program, &args, &plugin_dir, timeout))
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
        assert_eq!(m.output.mode, "text"); // default
        assert_eq!(m.timeout_secs, DEFAULT_TIMEOUT_SECS); // default
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
        // {input} without an input file is an error
        assert!(substitute_arg("{input}", dir, None, None).is_err());
        assert!(substitute_arg("{output}", dir, None, None).is_err());
    }

    #[test]
    fn test_resolve_command() {
        let dir = std::env::temp_dir();
        // Bare command falls back to PATH lookup (executable string unchanged)
        assert_eq!(resolve_command(&dir, "python3"), "python3");
        // Absolute path passes through
        assert_eq!(
            resolve_command(&dir, "/usr/local/bin/tool"),
            "/usr/local/bin/tool"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_run_process_echo() {
        let result =
            run_process("/bin/echo", &["hello".to_string()], Path::new("/tmp"), Duration::from_secs(5))
                .unwrap();
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("hello"));
        assert!(!result.timed_out);
    }

    #[cfg(unix)]
    #[test]
    fn test_run_process_timeout() {
        let result =
            run_process("/bin/sleep", &["10".to_string()], Path::new("/tmp"), Duration::from_secs(1))
                .unwrap();
        assert!(result.timed_out);
        assert_eq!(result.exit_code, None);
    }
}
