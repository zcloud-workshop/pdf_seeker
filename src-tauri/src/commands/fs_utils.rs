use crate::error::AppResult;

/// Recursively list files under `dir` whose extension matches one of
/// `extensions` (case-insensitive), sorted by path.
#[tauri::command]
pub fn list_dir_files(dir: String, extensions: Vec<String>) -> AppResult<Vec<String>> {
    let exts: Vec<String> = extensions.iter().map(|e| e.to_lowercase()).collect();
    let mut result = Vec::new();
    walk(std::path::Path::new(&dir), &exts, &mut result, 0)?;
    result.sort();
    Ok(result)
}

fn walk(dir: &std::path::Path, exts: &[String], out: &mut Vec<String>, depth: usize) -> std::io::Result<()> {
    if depth > 8 {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, exts, out, depth + 1)?;
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if exts.contains(&ext.to_lowercase()) {
                out.push(path.to_string_lossy().to_string());
            }
        }
    }
    Ok(())
}
