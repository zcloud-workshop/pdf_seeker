use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

/// Validates and canonicalizes a file path.
///
/// - Rejects paths containing `..` components in the original input
///   (before canonicalization) to prevent directory traversal.
/// - Canonicalizes the path to resolve symlinks, `.`, and redundant separators.
/// - Returns the canonicalized `PathBuf` on success.
/// - Returns `AppError::Io` if the path cannot be canonicalized.
pub fn validate_path(path: &str) -> AppResult<PathBuf> {
    let path = Path::new(path);

    // Reject any path component that is ".." in the original input.
    // This prevents directory traversal even before canonicalization.
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return Err(AppError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Path contains '..' component which is not allowed",
            )));
        }
    }

    // Canonicalize to resolve symlinks, `.`, and produce an absolute path.
    // This can fail if the path does not exist; callers that need to validate
    // a path for *creation* (e.g., an output file) should use
    // `validate_output_path` or handle the missing-file case separately.
    let canonical = path
        .canonicalize()
        .map_err(|e| std::io::Error::new(e.kind(), format!("Failed to canonicalize path: {e}")))?;

    Ok(canonical)
}

/// Validates an output path and ensures it falls within one of the allowed directories.
///
/// Calls [`validate_path`] first, then checks that the canonicalized path is a
/// descendant of (or equal to) one of the `allowed_dirs`.
pub fn validate_output_path(path: &str, allowed_dirs: &[PathBuf]) -> AppResult<PathBuf> {
    let canonical = validate_path(path)?;

    for allowed_dir in allowed_dirs {
        // Canonicalize the allowed directory so we are comparing like-for-like.
        let canonical_dir = allowed_dir
            .canonicalize()
            .map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Failed to canonicalize allowed directory {:?}: {e}", allowed_dir),
                )
            })?;

        if canonical.starts_with(&canonical_dir) {
            return Ok(canonical);
        }
    }

    Err(AppError::Io(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        format!(
            "Output path {:?} is outside all allowed directories",
            canonical
        ),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_validate_path_rejects_parent_dir() {
        let result = validate_path("/tmp/some/../etc/passwd");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains(".."),
            "Error message should mention '..': {err}"
        );
    }

    #[test]
    fn test_validate_path_rejects_trailing_parent() {
        let result = validate_path("/tmp/some/..");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_path_canonicalizes_existing() {
        // Use a path that is guaranteed to exist on any Unix-like system.
        let result = validate_path("/tmp");
        assert!(result.is_ok());
        let canonical = result.unwrap();
        assert!(canonical.is_absolute());
        // Canonicalized /tmp should not contain any "." or ".." components.
        assert!(!canonical.to_string_lossy().contains(".."));
    }

    #[test]
    fn test_validate_output_path_within_allowed() {
        let tmp = std::env::temp_dir();
        // Create a file inside temp so canonicalization succeeds.
        let test_file = tmp.join("pdf_seeker_validation_test.txt");
        fs::write(&test_file, "test").unwrap();

        let result = validate_output_path(
            test_file.to_str().unwrap(),
            &[tmp.clone()],
        );
        assert!(result.is_ok());

        // Clean up.
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_validate_output_path_outside_allowed() {
        let tmp = std::env::temp_dir();
        // Create a file inside temp.
        let test_file = tmp.join("pdf_seeker_validation_test_outside.txt");
        fs::write(&test_file, "test").unwrap();

        // Provide a different allowed directory (the binary's own directory,
        // which is almost certainly not /tmp).
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("/usr/bin"));

        let result = validate_output_path(
            test_file.to_str().unwrap(),
            &[exe_dir],
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("outside all allowed directories"),
            "Error message should mention allowed directories: {err}"
        );

        // Clean up.
        let _ = fs::remove_file(&test_file);
    }
}
