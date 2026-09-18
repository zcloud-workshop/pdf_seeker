//! Transactional PDF write layer.
//!
//! Guarantees:
//! 1. Original input is never modified.
//! 2. Output is written to a temporary file first.
//! 3. The temporary file is reopened and validated before committing.
//! 4. An atomic rename commits the validated output.
//! 5. On any failure the temporary file is cleaned up and the original is untouched.

use lopdf::{Document, ObjectId};
use std::path::Path;
use std::time::Instant;

use crate::error::{AppError, AppResult};

/// Policy for validating a written PDF before committing.
#[derive(Debug, Clone, Default)]
pub struct ValidationPolicy {
    /// If set, assert that the reopened document has exactly this many pages.
    pub expected_pages: Option<usize>,
    /// If true, verify the page tree structure (Kids array length matches Count).
    pub check_page_tree: bool,
}

/// Result metadata from a successful transactional write.
#[derive(Debug, Clone)]
pub struct WriteResult {
    /// Size of the output file in bytes.
    pub output_size: u64,
    /// Wall-clock duration of the write + validate + commit cycle.
    pub elapsed: std::time::Duration,
}

/// RAII guard that ensures a temporary file is deleted if not defused.
struct TempFileGuard {
    path: std::path::PathBuf,
    defused: bool,
}

impl TempFileGuard {
    fn new(path: std::path::PathBuf) -> Self {
        Self {
            path,
            defused: false,
        }
    }

    /// Defuse the guard so the temp file is NOT deleted on drop.
    fn defuse(&mut self) {
        self.defused = true;
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if !self.defused {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Load a PDF document from the given path.
pub fn load_doc(path: &str) -> AppResult<Document> {
    Document::load(path).map_err(|e| AppError::Pdf(format!("Load '{}': {}", path, e)))
}

/// Check if a document is encrypted and return an error if so.
///
/// Call this after `load_doc` on any write-path command to prevent
/// producing unreadable output from encrypted inputs.
pub fn reject_encrypted(doc: &Document, path: &str) -> AppResult<()> {
    if doc.is_encrypted() {
        return Err(AppError::Encrypted(path.to_string()));
    }
    Ok(())
}

/// Write a PDF document transactionally with validation.
///
/// The document is:
/// 1. Saved to a unique temporary file in the same directory as `output_path`.
/// 2. Reopened with `lopdf` to verify structural integrity.
/// 3. Validated against `policy` (page count, page tree).
/// 4. Atomically renamed to the final `output_path`.
///
/// On any failure, the temporary file is cleaned up and the original is untouched.
pub fn write_transactional(
    doc: &mut Document,
    output_path: &str,
    policy: &ValidationPolicy,
) -> AppResult<WriteResult> {
    let start = Instant::now();
    let output = Path::new(output_path);

    let parent = output
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .ok_or_else(|| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Output path must have a parent directory",
            ))
        })?;
    let file_name = output
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Output path must include a file name",
            ))
        })?;

    // Generate unique temporary file name
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| AppError::Pdf(format!("Create output nonce: {}", e)))?
        .as_nanos();
    let temporary = parent.join(format!(
        ".{}.pdf-seeker-{}-{}.tmp",
        file_name.to_string_lossy(),
        std::process::id(),
        nonce
    ));

    let mut guard = TempFileGuard::new(temporary.clone());

    // Step 1: Save to temporary file
    doc.save(&temporary)
        .map_err(|e| AppError::Pdf(format!("Save temporary '{}': {}", temporary.display(), e)))?;

    // Step 2: Reopen and validate
    let reopened = Document::load(&temporary).map_err(|e| {
        AppError::Pdf(format!(
            "Validation failed — reopening temporary PDF '{}': {}",
            temporary.display(),
            e
        ))
    })?;

    // Step 3: Apply validation policy
    if let Some(expected) = policy.expected_pages {
        let actual = reopened.get_pages().len();
        if actual != expected {
            return Err(AppError::Pdf(format!(
                "Page count mismatch: expected {} pages, got {} in '{}'",
                expected,
                actual,
                temporary.display()
            )));
        }
    }

    if policy.check_page_tree {
        validate_page_tree(&reopened)?;
    }

    // Step 4: Atomic commit via rename
    std::fs::rename(&temporary, output).map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!(
                "Commit validated PDF '{}' to '{}': {}",
                temporary.display(),
                output.display(),
                e
            ),
        ))
    })?;

    // Success — defuse the cleanup guard
    guard.defuse();

    let output_size = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);

    Ok(WriteResult {
        output_size,
        elapsed: start.elapsed(),
    })
}

/// Recursively count leaves in a page tree and validate internal node counts.
fn count_page_tree_leaves(
    doc: &Document,
    node_id: ObjectId,
    depth: usize,
    visited: &mut std::collections::HashSet<ObjectId>,
) -> AppResult<usize> {
    if depth > 100 {
        return Err(AppError::Pdf(
            "Page tree validation failed: maximum hierarchy depth exceeded (>100)".to_string(),
        ));
    }
    if !visited.insert(node_id) {
        return Err(AppError::Pdf(format!(
            "Page tree validation failed: cyclic reference detected at {:?}",
            node_id
        )));
    }

    let obj = doc
        .get_object(node_id)
        .map_err(|e| AppError::Pdf(format!("Page tree node {:?} not found: {}", node_id, e)))?;

    let dict = obj.as_dict().map_err(|e| {
        AppError::Pdf(format!(
            "Page tree node {:?} is not a dictionary: {}",
            node_id, e
        ))
    })?;

    let node_type = dict.get(b"Type").and_then(|o| o.as_name()).unwrap_or(b"");

    if node_type == b"Page" {
        return Ok(1);
    }

    if let Ok(kids) = dict.get(b"Kids").and_then(|o| o.as_array()) {
        let mut total_leaves = 0;
        for kid_obj in kids {
            let kid_id = match kid_obj {
                lopdf::Object::Reference(id) => *id,
                _ => {
                    return Err(AppError::Pdf(format!(
                        "Page tree node {:?} has non-reference child in Kids",
                        node_id
                    )));
                }
            };
            total_leaves += count_page_tree_leaves(doc, kid_id, depth + 1, visited)?;
        }

        if let Ok(count) = dict.get(b"Count").and_then(|o| o.as_i64()) {
            if count >= 0 && count as usize != total_leaves {
                return Err(AppError::Pdf(format!(
                    "Page tree inconsistent at node {:?}: Count={} but descendant leaves={}",
                    node_id, count, total_leaves
                )));
            }
        }

        Ok(total_leaves)
    } else if node_type == b"Pages" {
        let count = dict.get(b"Count").and_then(|o| o.as_i64()).unwrap_or(0);
        if count == 0 {
            Ok(0)
        } else {
            Err(AppError::Pdf(format!(
                "Page tree node {:?} has /Type /Pages but no Kids, yet Count={}",
                node_id, count
            )))
        }
    } else if dict.has(b"MediaBox") || dict.has(b"Parent") {
        Ok(1)
    } else {
        Err(AppError::Pdf(format!(
            "Page tree node {:?} is neither Page nor Pages",
            node_id
        )))
    }
}

/// Validate that the page tree is internally consistent.
/// Supports both flat page trees and hierarchical B-trees (such as those generated by Stirling-PDF / Ghostscript).
fn validate_page_tree(doc: &Document) -> AppResult<()> {
    let root_ref = doc
        .trailer
        .get(b"Root")
        .and_then(|o| o.as_reference())
        .map_err(|e| AppError::Pdf(format!("Page tree validation — Root: {}", e)))?;

    let pages_ref = doc
        .get_object(root_ref)
        .and_then(|o| o.as_dict())
        .and_then(|d| d.get(b"Pages"))
        .and_then(|o| o.as_reference())
        .map_err(|e| AppError::Pdf(format!("Page tree validation — Pages: {}", e)))?;

    let mut visited = std::collections::HashSet::new();
    let leaf_count = count_page_tree_leaves(doc, pages_ref, 0, &mut visited)?;

    // Check root Count if present
    if let Ok(pages_dict) = doc.get_object(pages_ref).and_then(|o| o.as_dict()) {
        if let Ok(count) = pages_dict.get(b"Count").and_then(|o| o.as_i64()) {
            if count >= 0 && leaf_count != count as usize {
                return Err(AppError::Pdf(format!(
                    "Page tree inconsistent: Root Count={} but traversed leaves={}",
                    count, leaf_count
                )));
            }
        }
    }

    // Verify consistency with lopdf's own page catalog
    let lopdf_pages_len = doc.get_pages().len();
    if lopdf_pages_len != leaf_count {
        return Err(AppError::Pdf(format!(
            "Page tree inconsistent: lopdf indexed {} pages but page tree has {} leaves",
            lopdf_pages_len, leaf_count
        )));
    }

    Ok(())
}

/// Verify that the output path does not match any input path.
pub fn ensure_distinct_output(output_path: &str, input_paths: &[&str]) -> AppResult<()> {
    use crate::commands::validation::{validate_output_path, validate_path};

    let output = validate_output_path(output_path)?;
    for input_path in input_paths {
        let input = validate_path(input_path)?;
        if input == output {
            return Err(AppError::Pdf(format!(
                "Output path '{}' must differ from input path '{}'",
                output.display(),
                input.display()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::Object;
    use tempfile::TempDir;

    /// Create a minimal valid PDF for testing.
    fn create_test_doc(num_pages: u32) -> Document {
        let mut doc = Document::with_version("1.4");
        let catalog_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
        let pages_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
            (b"Count".to_vec(), Object::Integer(num_pages as i64)),
            (b"Kids".to_vec(), Object::Array(vec![])),
        ])));

        if let Some(cat) = doc.objects.get_mut(&catalog_id) {
            if let Ok(d) = cat.as_dict_mut() {
                d.set("Type", Object::Name(b"Catalog".to_vec()));
                d.set("Pages", Object::Reference(pages_id));
            }
        }

        let mut kids = Vec::new();
        for _ in 0..num_pages {
            let page_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                (b"Type".to_vec(), Object::Name(b"Page".to_vec())),
                (b"Parent".to_vec(), Object::Reference(pages_id)),
                (
                    b"MediaBox".to_vec(),
                    Object::Array(vec![
                        Object::Integer(0),
                        Object::Integer(0),
                        Object::Integer(612),
                        Object::Integer(792),
                    ]),
                ),
            ])));
            kids.push(Object::Reference(page_id));
        }

        if let Some(pages_obj) = doc.objects.get_mut(&pages_id) {
            if let Ok(d) = pages_obj.as_dict_mut() {
                d.set("Kids", Object::Array(kids));
            }
        }

        doc.trailer.set(b"Root", Object::Reference(catalog_id));
        doc
    }

    #[test]
    fn test_write_transactional_basic() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("output.pdf");
        let mut doc = create_test_doc(3);

        let result = write_transactional(
            &mut doc,
            output.to_str().unwrap(),
            &ValidationPolicy::default(),
        )
        .unwrap();

        assert!(output.exists());
        assert!(result.output_size > 0);
        assert!(Document::load(&output).is_ok());
    }

    #[test]
    fn test_write_transactional_validates_page_count() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("output.pdf");
        let mut doc = create_test_doc(3);

        let policy = ValidationPolicy {
            expected_pages: Some(3),
            ..Default::default()
        };
        let result = write_transactional(&mut doc, output.to_str().unwrap(), &policy);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_transactional_rejects_wrong_page_count() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("output.pdf");
        let mut doc = create_test_doc(3);

        let policy = ValidationPolicy {
            expected_pages: Some(5),
            ..Default::default()
        };
        let result = write_transactional(&mut doc, output.to_str().unwrap(), &policy);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Page count mismatch"));
        // Temp file should have been cleaned up
        assert!(!output.exists());
    }

    #[test]
    fn test_write_transactional_checks_page_tree() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("output.pdf");
        let mut doc = create_test_doc(2);

        let policy = ValidationPolicy {
            check_page_tree: true,
            ..Default::default()
        };
        let result = write_transactional(&mut doc, output.to_str().unwrap(), &policy);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_transactional_no_temp_file_on_failure() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("output.pdf");
        let mut doc = create_test_doc(3);

        let policy = ValidationPolicy {
            expected_pages: Some(99), // Deliberately wrong
            ..Default::default()
        };
        let _ = write_transactional(&mut doc, output.to_str().unwrap(), &policy);

        // No temp files should remain
        let temp_files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("pdf-seeker"))
            .collect();
        assert!(temp_files.is_empty(), "Temp files should be cleaned up");
    }

    #[test]
    fn test_load_doc_success() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.pdf");
        let mut doc = create_test_doc(1);
        doc.save(&path).unwrap();

        let loaded = load_doc(path.to_str().unwrap());
        assert!(loaded.is_ok());
    }

    #[test]
    fn test_load_doc_missing_file() {
        let result = load_doc("/nonexistent/path/to/file.pdf");
        assert!(result.is_err());
    }

    #[test]
    fn test_reject_encrypted_on_normal_doc() {
        let doc = create_test_doc(1);
        let result = reject_encrypted(&doc, "test.pdf");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_hierarchical_page_tree_stirling_pdf() {
        // Construct a document with Root Count=16, but Root Kids has 2 intermediate nodes (8 pages each).
        // This reproduces the structure produced by Stirling-PDF / Ghostscript.
        let mut doc = Document::with_version("1.4");
        let catalog_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
        let root_pages_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
            (b"Count".to_vec(), Object::Integer(16)),
            (b"Kids".to_vec(), Object::Array(vec![])),
        ])));

        if let Some(cat) = doc.objects.get_mut(&catalog_id) {
            if let Ok(d) = cat.as_dict_mut() {
                d.set("Type", Object::Name(b"Catalog".to_vec()));
                d.set("Pages", Object::Reference(root_pages_id));
            }
        }
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut intermediate_ids = Vec::new();
        for _ in 0..2 {
            let inter_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
                (b"Parent".to_vec(), Object::Reference(root_pages_id)),
                (b"Count".to_vec(), Object::Integer(8)),
                (b"Kids".to_vec(), Object::Array(vec![])),
            ])));

            let mut page_refs = Vec::new();
            for _ in 0..8 {
                let page_id =
                    doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                        (b"Type".to_vec(), Object::Name(b"Page".to_vec())),
                        (b"Parent".to_vec(), Object::Reference(inter_id)),
                        (
                            b"MediaBox".to_vec(),
                            Object::Array(vec![
                                Object::Integer(0),
                                Object::Integer(0),
                                Object::Integer(612),
                                Object::Integer(792),
                            ]),
                        ),
                    ])));
                page_refs.push(Object::Reference(page_id));
            }

            if let Some(inter_obj) = doc.objects.get_mut(&inter_id) {
                if let Ok(d) = inter_obj.as_dict_mut() {
                    d.set("Kids", Object::Array(page_refs));
                }
            }

            intermediate_ids.push(Object::Reference(inter_id));
        }

        if let Some(root_obj) = doc.objects.get_mut(&root_pages_id) {
            if let Ok(d) = root_obj.as_dict_mut() {
                d.set("Kids", Object::Array(intermediate_ids));
            }
        }

        // Validate directly:
        let validation_result = validate_page_tree(&doc);
        assert!(
            validation_result.is_ok(),
            "Hierarchical page tree should be valid"
        );

        // Transactional write with check_page_tree = true
        let dir = TempDir::new().unwrap();
        let out_path = dir.path().join("hierarchical.pdf");
        let write_result = write_transactional(
            &mut doc,
            out_path.to_str().unwrap(),
            &ValidationPolicy {
                expected_pages: Some(16),
                check_page_tree: true,
            },
        );
        assert!(write_result.is_ok());

        // Now test that a genuine inconsistency in an intermediate node is detected
        let mut corrupted_doc = doc.clone();
        // Set intermediate node Count to 5 instead of 8
        if let Object::Reference(first_inter_id) = corrupted_doc
            .get_object(root_pages_id)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Kids")
            .unwrap()
            .as_array()
            .unwrap()[0]
        {
            if let Some(obj) = corrupted_doc.objects.get_mut(&first_inter_id) {
                if let Ok(d) = obj.as_dict_mut() {
                    d.set("Count", Object::Integer(5));
                }
            }
        }
        let corrupted_res = validate_page_tree(&corrupted_doc);
        assert!(corrupted_res.is_err());
        assert!(corrupted_res
            .unwrap_err()
            .to_string()
            .contains("Page tree inconsistent"));
    }
}
