use crate::error::AppResult;

/// Maximum directory nesting below the scanned root that `list_dir_files`
/// supports.
const MAX_DEPTH: usize = 8;

/// Recursively list files under `dir` whose extension matches one of
/// `extensions` (case-insensitive), sorted by path.
///
/// Walk rules:
/// - Only real subdirectories are followed. Symbolic links are never
///   dereferenced, so the walk cannot loop or leave `dir` through a link.
///   A symlink whose name matches is listed like any other entry, by its
///   link path.
/// - Subdirectories nested deeper than `MAX_DEPTH` levels below `dir`
///   cannot be reported through the `Vec<String>` return value, so the
///   walk fails with a readable error naming the offending directory
///   instead of silently returning a truncated result.
#[tauri::command]
pub fn list_dir_files(dir: String, extensions: Vec<String>) -> AppResult<Vec<String>> {
    let exts: Vec<String> = extensions.iter().map(|e| e.to_lowercase()).collect();
    let mut result = Vec::new();
    walk(std::path::Path::new(&dir), &exts, &mut result, 0)?;
    result.sort();
    Ok(result)
}

fn walk(
    dir: &std::path::Path,
    exts: &[String],
    out: &mut Vec<String>,
    depth: usize,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        // file_type() classifies the entry itself, unlike Path::is_dir(),
        // which follows a symlink to its target.
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            if depth >= MAX_DEPTH {
                return Err(std::io::Error::other(format!(
                    "directory nesting below the scanned root exceeds the supported depth of {MAX_DEPTH} levels: {}",
                    path.display()
                )));
            }
            walk(&path, exts, out, depth + 1)?;
        } else if file_type.is_file() || file_type.is_symlink() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if exts.contains(&ext.to_lowercase()) {
                    out.push(path.to_string_lossy().to_string());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{list_dir_files, MAX_DEPTH};
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};

    fn touch(dir: &Path, rel: &str) -> io::Result<PathBuf> {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, b"")?;
        Ok(path)
    }

    fn list(dir: &Path) -> Result<Vec<String>, String> {
        list_dir_files(dir.to_string_lossy().to_string(), vec!["pdf".to_string()])
            .map_err(|e| e.to_string())
    }

    fn names(files: &[String]) -> Vec<&str> {
        files
            .iter()
            .map(|p| Path::new(p).file_name().unwrap().to_str().unwrap())
            .collect()
    }

    #[test]
    fn lists_matching_files_recursively_sorted_case_insensitively() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        touch(root.path(), "b.pdf")?;
        touch(root.path(), "a.txt")?;
        touch(root.path(), "sub/B.PDF")?;
        touch(root.path(), "sub/ignore.md")?;
        touch(root.path(), "sub/deep/c.pdf")?;

        let files = list(root.path()).unwrap();
        assert_eq!(names(&files), ["b.pdf", "B.PDF", "c.pdf"]);
        assert!(
            files.windows(2).all(|w| w[0] <= w[1]),
            "result must be sorted by path"
        );
        Ok(())
    }

    #[test]
    fn empty_root_is_ok_and_missing_root_errors() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        assert_eq!(list(root.path()).unwrap(), Vec::<String>::new());

        assert!(list(&root.path().join("nope")).is_err());
        Ok(())
    }

    #[test]
    fn errors_instead_of_truncating_when_depth_exceeded() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let mut dir = root.path().to_path_buf();
        for _ in 0..=MAX_DEPTH {
            dir = dir.join("d");
            fs::create_dir(&dir)?;
        }
        touch(&dir, "deep.pdf")?;

        let err = list(root.path()).unwrap_err();
        assert!(
            err.contains("depth"),
            "error must explain the depth limit, got: {err}"
        );
        assert!(
            err.contains("d/d"),
            "error must name the directory that exceeded the limit, got: {err}"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn directory_symlinks_are_not_followed() -> io::Result<()> {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir()?;
        touch(root.path(), "sub/inner.pdf")?;

        // Cycle: link inside the tree pointing back at the root.
        symlink(root.path(), root.path().join("sub/loop"))?;
        // Escape: link pointing to a directory outside the scanned root.
        let outside = tempfile::tempdir()?;
        touch(outside.path(), "outside.pdf")?;
        symlink(outside.path(), root.path().join("sub/escape"))?;

        let files = list(root.path()).unwrap();
        assert_eq!(names(&files), ["inner.pdf"]);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_file_is_listed_by_its_link_name() -> io::Result<()> {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir()?;
        let target = touch(root.path(), "hidden-target.bin")?;
        symlink(&target, root.path().join("linked.pdf"))?;

        let files = list(root.path()).unwrap();
        assert_eq!(names(&files), ["linked.pdf"]);
        Ok(())
    }
}
