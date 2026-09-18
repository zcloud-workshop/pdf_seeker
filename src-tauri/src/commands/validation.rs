use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

fn reject_parent_components(path: &Path) -> AppResult<()> {
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return Err(AppError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Path contains '..' component which is not allowed",
            )));
        }
    }
    Ok(())
}

/// Validates an existing input path.
///
/// Input files and directories must exist so symbolic links can be resolved and
/// callers receive a clear error before attempting a PDF operation.
pub fn validate_path(path: &str) -> AppResult<PathBuf> {
    let path = Path::new(path);
    reject_parent_components(path)?;

    path.canonicalize().map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to canonicalize existing path: {e}"),
        ))
    })
}

/// Validates a final output file path without requiring the file to exist.
///
/// The parent directory must already exist and is canonicalized before the
/// filename is joined. This supports normal Save As behavior while retaining
/// traversal protection and rejecting missing output directories.
pub fn validate_output_path(path: &str) -> AppResult<PathBuf> {
    let path = Path::new(path);
    reject_parent_components(path)?;

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Output path must have a parent directory",
            ))
        })?;
    let file_name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Output path must include a file name",
            ))
        })?;
    let canonical_parent = parent.canonicalize().map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to canonicalize output parent directory: {e}"),
        ))
    })?;

    Ok(canonical_parent.join(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_rejects_parent_dir() {
        let result = validate_path("/tmp/some/../etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(".."));
    }

    #[test]
    fn test_validate_path_rejects_trailing_parent() {
        assert!(validate_path("/tmp/some/..").is_err());
    }

    #[test]
    fn test_validate_path_canonicalizes_existing() {
        let canonical = validate_path("/tmp").unwrap();
        assert!(canonical.is_absolute());
        assert!(!canonical.to_string_lossy().contains(".."));
    }

    #[test]
    fn test_validate_path_rejects_missing_input() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(validate_path(dir.path().join("missing.pdf").to_str().unwrap()).is_err());
    }

    #[test]
    fn test_validate_output_path_accepts_new_file_in_existing_parent() {
        let dir = tempfile::TempDir::new().unwrap();
        let output = dir.path().join("new-output.pdf");

        let canonical_parent = dir.path().canonicalize().unwrap();
        let expected = canonical_parent.join("new-output.pdf");
        let validated = validate_output_path(output.to_str().unwrap()).unwrap();
        assert_eq!(validated, expected);
        assert!(!validated.exists());
    }

    #[test]
    fn test_validate_output_path_rejects_missing_parent() {
        let dir = tempfile::TempDir::new().unwrap();
        let output = dir.path().join("missing-parent").join("new-output.pdf");
        assert!(validate_output_path(output.to_str().unwrap()).is_err());
    }

    #[test]
    fn test_validate_output_path_rejects_parent_dir() {
        assert!(validate_output_path("/tmp/some/../new-output.pdf").is_err());
    }
}
