//! PDF operations facade and comprehensive regression test suite.
//!
//! Specific PDF commands are implemented across dedicated submodules:
//! - [`crate::commands::organize`]: Page-level manipulation (merge, rotate, delete, split, extract, reorder, insert)
//! - [`crate::commands::annotate`]: Annotations and visual editing (watermark, sign, text, rect, highlight, whiteout, batch edits)
//! - [`crate::commands::convert`]: Conversion and text extraction (images-to-PDF, extract text)
//! - [`crate::commands::info`]: Document inspection and compression (PDF info, compress, temp dir, image saving)

pub use crate::commands::annotate::*;
pub use crate::commands::convert::*;
pub use crate::commands::info::*;
pub use crate::commands::organize::*;

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Document, Object, ObjectId};
    use tempfile::TempDir;

    /// Create a minimal valid multi-page PDF for unit tests.
    fn create_test_pdf(dir: &std::path::Path, name: &str, num_pages: u32) -> String {
        let path = dir.join(name);
        let mut doc = Document::with_version("1.4");

        let catalog_id = doc.add_object(lopdf::Object::Dictionary(lopdf::Dictionary::new()));
        let pages_id = doc.add_object(lopdf::Object::Dictionary(lopdf::Dictionary::from_iter(
            vec![
                (b"Type".to_vec(), lopdf::Object::Name(b"Pages".to_vec())),
                (b"Count".to_vec(), lopdf::Object::Integer(num_pages as i64)),
                (b"Kids".to_vec(), lopdf::Object::Array(vec![])),
            ],
        )));

        if let Some(cat) = doc.objects.get_mut(&catalog_id) {
            if let Ok(d) = cat.as_dict_mut() {
                d.set("Type", lopdf::Object::Name(b"Catalog".to_vec()));
                d.set("Pages", lopdf::Object::Reference(pages_id));
            }
        }

        let mut kids = Vec::new();
        for _ in 0..num_pages {
            let page_id = doc.add_object(lopdf::Object::Dictionary(lopdf::Dictionary::from_iter(
                vec![
                    (b"Type".to_vec(), lopdf::Object::Name(b"Page".to_vec())),
                    (b"Parent".to_vec(), lopdf::Object::Reference(pages_id)),
                    (
                        b"MediaBox".to_vec(),
                        lopdf::Object::Array(vec![
                            lopdf::Object::Integer(0),
                            lopdf::Object::Integer(0),
                            lopdf::Object::Integer(612),
                            lopdf::Object::Integer(792),
                        ]),
                    ),
                ],
            )));
            kids.push(lopdf::Object::Reference(page_id));
        }

        if let Some(pages_obj) = doc.objects.get_mut(&pages_id) {
            if let Ok(d) = pages_obj.as_dict_mut() {
                d.set("Kids", lopdf::Object::Array(kids));
            }
        }

        doc.trailer
            .set(b"Root", lopdf::Object::Reference(catalog_id));
        doc.save(&path).unwrap();
        path.to_string_lossy().to_string()
    }

    // ─── Unit tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_merge_two_pdfs() {
        let dir = TempDir::new().unwrap();
        let p1 = create_test_pdf(dir.path(), "a.pdf", 2);
        let p2 = create_test_pdf(dir.path(), "b.pdf", 3);
        let out = dir.path().join("merged.pdf");
        let out_str = out.to_string_lossy().to_string();

        merge_pdfs(vec![p1, p2], out_str.clone()).unwrap();

        let doc = Document::load(&out_str).unwrap();
        assert_eq!(doc.get_pages().len(), 5);
    }

    #[test]
    fn test_rotate_pdf() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "r.pdf", 2);
        let out = dir.path().join("rotated.pdf");
        let out_str = out.to_string_lossy().to_string();

        rotate_pdf(RotatePdfRequest {
            input_path: src,
            output_path: out_str.clone(),
            angle: 90,
        })
        .unwrap();

        let doc = Document::load(&out_str).unwrap();
        assert_eq!(doc.get_pages().len(), 2);
        for (_, id) in doc.get_pages() {
            let obj = doc.get_object(id).unwrap();
            if let Ok(dict) = obj.as_dict() {
                if let Ok(rot_obj) = dict.get(b"Rotate") {
                    if let Ok(rot) = rot_obj.as_i64() {
                        assert_eq!(rot, 90);
                    }
                }
            }
        }
    }

    #[test]
    fn test_rotate_rejects_output_matching_input() {
        let dir = TempDir::new().unwrap();
        let source = create_test_pdf(dir.path(), "same.pdf", 2);

        let result = rotate_pdf(RotatePdfRequest {
            input_path: source.clone(),
            output_path: source.clone(),
            angle: 90,
        });

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must differ"));
        assert_eq!(Document::load(source).unwrap().get_pages().len(), 2);
    }

    #[test]
    fn test_split_rejects_generated_output_matching_input() {
        let dir = TempDir::new().unwrap();
        let source = create_test_pdf(dir.path(), "page_1.pdf", 1);
        let output_dir = dir.path().to_string_lossy().to_string();

        let result = split_pdf(SplitPdfRequest {
            input_path: source.clone(),
            output_dir,
            mode: "single".into(),
            ranges: None,
        });

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must differ"));
        assert_eq!(Document::load(source).unwrap().get_pages().len(), 1);
    }

    #[test]
    fn test_delete_pages() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "d.pdf", 5);
        let out = dir.path().join("deleted.pdf");
        let out_str = out.to_string_lossy().to_string();

        delete_pages(DeletePagesRequest {
            input_path: src,
            output_path: out_str.clone(),
            pages_to_delete: vec![2, 4],
        })
        .unwrap();

        let doc = Document::load(&out_str).unwrap();
        assert_eq!(doc.get_pages().len(), 3);
    }

    #[test]
    fn test_extract_text() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "t.pdf", 1);

        let result = extract_text(src).unwrap();
        assert_eq!(result.pages, 1);
        assert!(result.text.contains("--- Page 1 ---"));
    }

    #[test]
    fn test_merge_empty_list_fails() {
        let err = merge_pdfs(vec![], "/tmp/nothing.pdf".into());
        assert!(err.is_err());
    }

    #[test]
    fn test_delete_all_pages() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "da.pdf", 3);
        let out = dir.path().join("del_all.pdf");
        let out_str = out.to_string_lossy().to_string();

        let result = delete_pages(DeletePagesRequest {
            input_path: src,
            output_path: out_str.clone(),
            pages_to_delete: vec![1, 2, 3],
        });
        if let Ok(()) = result {
            let doc = Document::load(&out_str).unwrap();
            assert_eq!(doc.get_pages().len(), 0);
        }
    }

    // ─── Fixture regression tests ───────────────────────────────────────────

    const SINGLE_PAGE_FIXTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/single-page-text.pdf"
    ));
    const THREE_PAGE_TARGET_FIXTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/three-page-target.pdf"
    ));
    const TWO_PAGE_SOURCE_FIXTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/two-page-source.pdf"
    ));
    const ROTATED_CONTENTS_ARRAY_FIXTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/rotated-contents-array.pdf"
    ));
    const IMAGE_FIXTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/two-by-two-rgba.png"
    ));
    const CORRUPTED_PDF_FIXTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/truncated-input.pdf"
    ));
    const CHINESE_TEXT_FIXTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/chinese-text.pdf"
    ));
    const ENCRYPTED_PDF_FIXTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/encrypted-test.pdf"
    ));

    fn write_fixture(dir: &std::path::Path, name: &str, bytes: &[u8]) -> String {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path.to_string_lossy().to_string()
    }

    fn prepare_output(dir: &std::path::Path, name: &str) -> String {
        dir.join(name).to_string_lossy().to_string()
    }

    fn assert_external_pdf_checks(path: &str) {
        if std::env::var_os("PDF_SEEKER_EXTERNAL_PDF_CHECKS").is_none() {
            return;
        }

        let qpdf = std::process::Command::new("qpdf")
            .arg("--check")
            .arg(path)
            .status()
            .expect("qpdf must be installed when external checks are enabled");
        assert!(qpdf.success(), "qpdf validation failed for {path}");

        let render_prefix = format!("{path}.render-check");
        let poppler = std::process::Command::new("pdftoppm")
            .args(["-f", "1", "-l", "1", "-png", "-singlefile"])
            .arg(path)
            .arg(&render_prefix)
            .status()
            .expect("pdftoppm must be installed when external checks are enabled");
        assert!(poppler.success(), "Poppler rendering failed for {path}");

        let rendered = format!("{render_prefix}.png");
        assert!(std::path::Path::new(&rendered).is_file());
        let _ = std::fs::remove_file(rendered);
    }

    #[test]
    fn regression_fixture_extracts_known_text() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "single.pdf", SINGLE_PAGE_FIXTURE);

        let result = extract_text(input).unwrap();
        assert_eq!(result.pages, 1);
        assert!(result.text.contains("PDF Seeker Fixture: single-page text"));
    }

    #[test]
    fn regression_fixture_images_to_pdf_reopens() {
        let dir = TempDir::new().unwrap();
        let image = write_fixture(dir.path(), "fixture.png", IMAGE_FIXTURE);
        let output = prepare_output(dir.path(), "image-output.pdf");

        images_to_pdf(ImagesToPdfRequest {
            image_paths: vec![image],
            output_path: output.clone(),
        })
        .unwrap();

        let doc = Document::load(&output).expect("image-to-PDF output must reopen");
        assert_eq!(doc.get_pages().len(), 1);
        assert_external_pdf_checks(&output);
    }

    #[test]
    fn regression_fixture_corrupted_input_fails_without_output() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "truncated.pdf", CORRUPTED_PDF_FIXTURE);
        let output = prepare_output(dir.path(), "should-not-exist.pdf");

        let result = rotate_pdf(RotatePdfRequest {
            input_path: input,
            output_path: output.clone(),
            angle: 90,
        });

        assert!(result.is_err());
        assert!(!std::path::Path::new(&output).exists());
    }

    #[test]
    fn regression_fixture_rotated_contents_array_preserves_all_streams() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(
            dir.path(),
            "rotated-contents-array.pdf",
            ROTATED_CONTENTS_ARRAY_FIXTURE,
        );
        let input_text = extract_text(input.clone()).unwrap().text;
        assert!(input_text.contains("content-array part one"));
        assert!(input_text.contains("content-array part two"));

        let output = prepare_output(dir.path(), "rotated-contents-array-output.pdf");
        rotate_pdf(RotatePdfRequest {
            input_path: input,
            output_path: output.clone(),
            angle: 90,
        })
        .unwrap();

        let doc = Document::load(&output).expect("rotated content-array PDF must reopen");
        assert_eq!(doc.get_pages().len(), 1);
        let page_id = *doc.get_pages().get(&1).unwrap();
        let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
        assert_eq!(page.get(b"Rotate").unwrap().as_i64().unwrap(), 180);
        assert_external_pdf_checks(&output);

        let output_text = extract_text(output).unwrap().text;
        assert!(output_text.contains("content-array part one"));
        assert!(output_text.contains("content-array part two"));
    }

    #[test]
    fn regression_fixture_rotation_reopens_and_preserves_page_count() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "target.pdf", THREE_PAGE_TARGET_FIXTURE);
        let output = prepare_output(dir.path(), "rotated.pdf");

        rotate_pdf(RotatePdfRequest {
            input_path: input,
            output_path: output.clone(),
            angle: 90,
        })
        .unwrap();

        let doc = Document::load(&output).expect("rotated regression PDF must reopen");
        assert_eq!(doc.get_pages().len(), 3);
        assert_external_pdf_checks(&output);
        for (_, page_id) in doc.get_pages() {
            let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
            assert_eq!(page.get(b"Rotate").unwrap().as_i64().unwrap(), 90);
        }
    }

    #[test]
    fn regression_fixture_insert_pages_reopens_with_ordered_content() {
        let dir = TempDir::new().unwrap();
        let target = write_fixture(dir.path(), "target.pdf", THREE_PAGE_TARGET_FIXTURE);
        let source = write_fixture(dir.path(), "source.pdf", TWO_PAGE_SOURCE_FIXTURE);
        let output = prepare_output(dir.path(), "inserted.pdf");

        insert_pages(InsertPagesRequest {
            input_path: target,
            source_path: source,
            output_path: output.clone(),
            insert_position: 1,
        })
        .unwrap();

        let doc = Document::load(&output).expect("inserted regression PDF must reopen");
        assert_eq!(doc.get_pages().len(), 5);
        assert_external_pdf_checks(&output);
        let text = extract_text(output).unwrap().text;
        assert!(text.contains("target page 1"));
        assert!(text.contains("source page 1"));
        assert!(text.contains("source page 2"));
    }

    #[test]
    fn regression_fixture_batch_edits_reopen_and_keep_text() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "editable.pdf", SINGLE_PAGE_FIXTURE);
        let output = prepare_output(dir.path(), "edited.pdf");

        apply_edit_operations(
            input,
            output.clone(),
            vec![
                EditOp {
                    op_type: "addText".into(),
                    params: serde_json::json!({
                        "page": 1,
                        "text": "Regression text",
                        "x": 72.0,
                        "y": 680.0,
                        "fontSize": 14.0,
                        "color": "#000000"
                    }),
                },
                EditOp {
                    op_type: "addRectangle".into(),
                    params: serde_json::json!({
                        "page": 1,
                        "x": 70.0,
                        "y": 650.0,
                        "w": 180.0,
                        "h": 30.0,
                        "borderColor": "#ff0000",
                        "hasFill": true,
                        "fillColor": "#ffeeee",
                        "borderWidth": 1.0
                    }),
                },
                EditOp {
                    op_type: "addHighlight".into(),
                    params: serde_json::json!({
                        "page": 1,
                        "x": 70.0,
                        "y": 710.0,
                        "w": 240.0,
                        "h": 20.0,
                        "color": "#ffff00",
                        "opacity": 0.4
                    }),
                },
            ],
        )
        .unwrap();

        let doc = Document::load(&output).expect("edited regression PDF must reopen");
        assert_eq!(doc.get_pages().len(), 1);
        assert_external_pdf_checks(&output);
        let text = extract_text(output).unwrap().text;
        assert!(text.contains("PDF Seeker Fixture: single-page text"));
        assert!(text.contains("Regression text"));
    }

    // ─── Extended Phase 0 tests: Core page operations ───────────────────────

    #[test]
    fn test_split_fixture() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "target.pdf", THREE_PAGE_TARGET_FIXTURE);
        let out_dir = dir.path().join("split_out");
        std::fs::create_dir_all(&out_dir).unwrap();

        let outputs = split_pdf(SplitPdfRequest {
            input_path: input,
            output_dir: out_dir.to_string_lossy().to_string(),
            mode: "single".into(),
            ranges: None,
        })
        .unwrap();

        assert_eq!(outputs.len(), 3);
        for (idx, out_path) in outputs.iter().enumerate() {
            let doc = Document::load(out_path).expect("split page must reopen");
            assert_eq!(doc.get_pages().len(), 1);
            let text = extract_text(out_path.clone()).unwrap().text;
            assert!(text.contains(&format!("target page {}", idx + 1)));
            assert_external_pdf_checks(out_path);
        }
    }

    #[test]
    fn test_extract_pages_fixture() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "target.pdf", THREE_PAGE_TARGET_FIXTURE);
        let output = prepare_output(dir.path(), "extracted.pdf");

        extract_pages_pdf(ExtractPagesRequest {
            input_path: input,
            output_path: output.clone(),
            pages_to_extract: vec![1, 3],
        })
        .unwrap();

        let doc = Document::load(&output).expect("extracted PDF must reopen");
        assert_eq!(doc.get_pages().len(), 2);
        let text = extract_text(output.clone()).unwrap().text;
        assert!(text.contains("target page 1"));
        assert!(!text.contains("target page 2"));
        assert!(text.contains("target page 3"));
        assert_external_pdf_checks(&output);
    }

    #[test]
    fn test_reorder_fixture() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "target.pdf", THREE_PAGE_TARGET_FIXTURE);
        let output = prepare_output(dir.path(), "reordered.pdf");

        reorder_pages(ReorderPagesRequest {
            input_path: input,
            output_path: output.clone(),
            new_order: vec![3, 1, 2],
        })
        .unwrap();

        let doc = Document::load(&output).expect("reordered PDF must reopen");
        assert_eq!(doc.get_pages().len(), 3);
        let text = extract_text(output.clone()).unwrap().text;
        let pos1 = text.find("target page 1").unwrap();
        let pos2 = text.find("target page 2").unwrap();
        let pos3 = text.find("target page 3").unwrap();
        assert!(pos3 < pos1);
        assert!(pos1 < pos2);
        assert_external_pdf_checks(&output);
    }

    #[test]
    fn test_merge_fixture_with_real_content() {
        let dir = TempDir::new().unwrap();
        let target = write_fixture(dir.path(), "target.pdf", THREE_PAGE_TARGET_FIXTURE);
        let source = write_fixture(dir.path(), "source.pdf", TWO_PAGE_SOURCE_FIXTURE);
        let output = prepare_output(dir.path(), "merged_real.pdf");

        merge_pdfs(vec![target, source], output.clone()).unwrap();

        let doc = Document::load(&output).expect("merged PDF must reopen");
        assert_eq!(doc.get_pages().len(), 5);
        let text = extract_text(output.clone()).unwrap().text;
        assert!(text.contains("target page 1"));
        assert!(text.contains("target page 3"));
        assert!(text.contains("source page 1"));
        assert!(text.contains("source page 2"));
        assert_external_pdf_checks(&output);
    }

    #[test]
    fn test_delete_pages_fixture() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "target.pdf", THREE_PAGE_TARGET_FIXTURE);
        let output = prepare_output(dir.path(), "deleted.pdf");

        delete_pages(DeletePagesRequest {
            input_path: input,
            output_path: output.clone(),
            pages_to_delete: vec![2],
        })
        .unwrap();

        let doc = Document::load(&output).expect("deleted page PDF must reopen");
        assert_eq!(doc.get_pages().len(), 2);
        let text = extract_text(output.clone()).unwrap().text;
        assert!(text.contains("target page 1"));
        assert!(!text.contains("target page 2"));
        assert!(text.contains("target page 3"));
        assert_external_pdf_checks(&output);
    }

    #[test]
    fn test_compress_fixture() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "target.pdf", THREE_PAGE_TARGET_FIXTURE);
        let output = prepare_output(dir.path(), "compressed.pdf");

        let res = compress_pdf(input, output.clone()).unwrap();
        assert!(res.original_size > 0);
        let doc = Document::load(&output).expect("compressed PDF must reopen");
        assert_eq!(doc.get_pages().len(), 3);
        assert_external_pdf_checks(&output);
    }

    #[test]
    fn test_watermark_fixture() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "single.pdf", SINGLE_PAGE_FIXTURE);
        let output = prepare_output(dir.path(), "watermarked.pdf");

        add_text_watermark(WatermarkRequest {
            input_path: input,
            output_path: output.clone(),
            text: "CONFIDENTIAL".into(),
            font_size: 36.0,
            opacity: 0.3,
            angle: 45.0,
            color: "#FF0000".into(),
        })
        .unwrap();

        let doc = Document::load(&output).expect("watermarked PDF must reopen");
        assert_eq!(doc.get_pages().len(), 1);
        let text = extract_text(output.clone()).unwrap().text;
        assert!(text.contains("PDF Seeker Fixture: single-page text"));
        assert_external_pdf_checks(&output);
    }

    // ─── Extended Phase 0 tests: Chinese text fixture ───────────────────────

    #[test]
    fn test_chinese_fixture_rotate_preserves_content() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "chinese.pdf", CHINESE_TEXT_FIXTURE);
        let output = prepare_output(dir.path(), "chinese_rotated.pdf");

        rotate_pdf(RotatePdfRequest {
            input_path: input,
            output_path: output.clone(),
            angle: 90,
        })
        .unwrap();

        let doc = Document::load(&output).expect("rotated Chinese PDF must reopen");
        assert_eq!(doc.get_pages().len(), 2);
        let text = extract_text(output.clone()).unwrap().text;
        assert!(text.contains("CHINESE-FIXTURE-MARK Page 1"));
        assert!(text.contains("CHINESE-FIXTURE-MARK Page 2"));
        assert_external_pdf_checks(&output);
    }

    #[test]
    fn test_chinese_fixture_merge_preserves_content() {
        let dir = TempDir::new().unwrap();
        let chinese = write_fixture(dir.path(), "chinese.pdf", CHINESE_TEXT_FIXTURE);
        let single = write_fixture(dir.path(), "single.pdf", SINGLE_PAGE_FIXTURE);
        let output = prepare_output(dir.path(), "chinese_merged.pdf");

        merge_pdfs(vec![chinese, single], output.clone()).unwrap();

        let doc = Document::load(&output).expect("merged Chinese PDF must reopen");
        assert_eq!(doc.get_pages().len(), 3);
        let text = extract_text(output.clone()).unwrap().text;
        assert!(text.contains("CHINESE-FIXTURE-MARK"));
        assert!(text.contains("PDF Seeker Fixture: single-page text"));
        assert_external_pdf_checks(&output);
    }

    // ─── Extended Phase 0 tests: Encrypted & Corrupted inputs ───────────────

    #[test]
    fn test_encrypted_pdf_info_detection() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "encrypted.pdf", ENCRYPTED_PDF_FIXTURE);

        let info = get_pdf_info(input).unwrap();
        assert!(info.is_encrypted, "must detect PDF encryption");
    }

    #[test]
    fn test_encrypted_pdf_rotate_fails_clearly() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "encrypted.pdf", ENCRYPTED_PDF_FIXTURE);
        let output = prepare_output(dir.path(), "should_not_exist.pdf");

        let result = rotate_pdf(RotatePdfRequest {
            input_path: input,
            output_path: output.clone(),
            angle: 90,
        });

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("password-protected") || err_msg.contains("encrypted"),
            "Error message must clearly state password protection: {err_msg}"
        );
        assert!(
            !std::path::Path::new(&output).exists(),
            "Output file must not be generated"
        );
    }

    #[test]
    fn test_corrupted_input_split_fails_safely() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "corrupted.pdf", CORRUPTED_PDF_FIXTURE);
        let out_dir = dir.path().join("split_fail_out");
        std::fs::create_dir_all(&out_dir).unwrap();

        let result = split_pdf(SplitPdfRequest {
            input_path: input,
            output_dir: out_dir.to_string_lossy().to_string(),
            mode: "single".into(),
            ranges: None,
        });

        assert!(result.is_err());
        let entries: Vec<_> = std::fs::read_dir(&out_dir).unwrap().collect();
        assert!(
            entries.is_empty(),
            "No split output should be left on corruption"
        );
    }

    #[test]
    fn test_corrupted_input_merge_fails_safely() {
        let dir = TempDir::new().unwrap();
        let input1 = write_fixture(dir.path(), "corrupted.pdf", CORRUPTED_PDF_FIXTURE);
        let input2 = write_fixture(dir.path(), "single.pdf", SINGLE_PAGE_FIXTURE);
        let output = prepare_output(dir.path(), "merge_fail.pdf");

        let result = merge_pdfs(vec![input1, input2], output.clone());
        assert!(result.is_err());
        assert!(
            !std::path::Path::new(&output).exists(),
            "Merge output must not exist on failure"
        );
    }

    #[test]
    fn test_add_page_numbers_success() {
        let dir = TempDir::new().unwrap();
        let input = create_test_pdf(dir.path(), "numbered_in.pdf", 3);
        let output = prepare_output(dir.path(), "numbered_out.pdf");

        let res = add_page_numbers(AddPageNumbersRequest {
            input_path: input,
            output_path: output.clone(),
            format: "Page {n} of {total}".into(),
            position: "bottom-center".into(),
            start_page: Some(1),
            start_number: Some(1),
            font_size: Some(11.0),
            margin: Some(25.0),
            color: Some("#333333".into()),
        });
        assert!(res.is_ok(), "add_page_numbers failed: {:?}", res.err());

        let doc = Document::load(&output).expect("load numbered pdf");
        assert_eq!(doc.get_pages().len(), 3);
    }

    #[test]
    fn test_sanitize_pdf_removes_info_and_metadata() {
        let dir = TempDir::new().unwrap();
        let input_path = dir.path().join("metadata_in.pdf");
        let mut doc = Document::with_version("1.4");
        let cat_id = doc.add_object(lopdf::Object::Dictionary(lopdf::Dictionary::new()));
        let pages_id = doc.add_object(lopdf::Object::Dictionary(lopdf::Dictionary::from_iter(
            vec![
                (b"Type".to_vec(), lopdf::Object::Name(b"Pages".to_vec())),
                (b"Count".to_vec(), lopdf::Object::Integer(1)),
                (b"Kids".to_vec(), lopdf::Object::Array(vec![])),
            ],
        )));
        let page_id = doc.add_object(lopdf::Object::Dictionary(lopdf::Dictionary::from_iter(
            vec![
                (b"Type".to_vec(), lopdf::Object::Name(b"Page".to_vec())),
                (b"Parent".to_vec(), lopdf::Object::Reference(pages_id)),
                (
                    b"MediaBox".to_vec(),
                    lopdf::Object::Array(vec![
                        lopdf::Object::Integer(0),
                        lopdf::Object::Integer(0),
                        lopdf::Object::Integer(612),
                        lopdf::Object::Integer(792),
                    ]),
                ),
            ],
        )));
        if let Some(pages_obj) = doc.objects.get_mut(&pages_id) {
            if let Ok(d) = pages_obj.as_dict_mut() {
                d.set(
                    "Kids",
                    lopdf::Object::Array(vec![lopdf::Object::Reference(page_id)]),
                );
            }
        }
        if let Some(cat) = doc.objects.get_mut(&cat_id) {
            if let Ok(d) = cat.as_dict_mut() {
                d.set("Type", lopdf::Object::Name(b"Catalog".to_vec()));
                d.set("Pages", lopdf::Object::Reference(pages_id));
            }
        }
        let info_id = doc.add_object(lopdf::Object::Dictionary(lopdf::Dictionary::from_iter(
            vec![
                (
                    b"Title".to_vec(),
                    lopdf::Object::String(b"Secret Title".to_vec(), lopdf::StringFormat::Literal),
                ),
                (
                    b"Author".to_vec(),
                    lopdf::Object::String(b"Secret Author".to_vec(), lopdf::StringFormat::Literal),
                ),
            ],
        )));
        doc.trailer.set(b"Root", lopdf::Object::Reference(cat_id));
        doc.trailer.set(b"Info", lopdf::Object::Reference(info_id));
        doc.save(&input_path).unwrap();

        let output = prepare_output(dir.path(), "sanitized_out.pdf");
        let res = sanitize_pdf(input_path.to_string_lossy().to_string(), output.clone());
        assert!(res.is_ok(), "sanitize_pdf failed: {:?}", res.err());

        let clean_doc = Document::load(&output).expect("load sanitized pdf");
        assert_eq!(clean_doc.get_pages().len(), 1);
        assert!(
            clean_doc.trailer.get(b"Info").is_err(),
            "Info dictionary must be removed"
        );
    }

    #[test]
    fn test_rotate_negative_angle_normalized_to_positive() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "rotate_neg_input.pdf", SINGLE_PAGE_FIXTURE);
        let output = prepare_output(dir.path(), "rotate_neg_output.pdf");

        rotate_pdf(RotatePdfRequest {
            input_path: input,
            output_path: output.clone(),
            angle: -90,
        })
        .unwrap();

        let doc = Document::load(&output).unwrap();
        let pages = doc.get_pages();
        let page_obj = doc.get_object(pages[&1]).unwrap();
        let rotate = page_obj
            .as_dict()
            .unwrap()
            .get(b"Rotate")
            .unwrap()
            .as_i64()
            .unwrap();
        assert_eq!(
            rotate, 270,
            "Negative 90 deg rotation must normalize to 270"
        );
    }

    #[test]
    fn test_delete_duplicate_pages_handled_safely() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "del_dup_input.pdf", THREE_PAGE_TARGET_FIXTURE);
        let output = prepare_output(dir.path(), "del_dup_output.pdf");

        delete_pages(DeletePagesRequest {
            input_path: input,
            output_path: output.clone(),
            pages_to_delete: vec![1, 1],
        })
        .unwrap();

        let doc = Document::load(&output).unwrap();
        assert_eq!(
            doc.get_pages().len(),
            2,
            "3-page doc after deleting page 1 (with duplicate in request) must have 2 pages"
        );
    }

    #[test]
    fn test_extract_duplicate_pages_handled_safely() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(dir.path(), "ext_dup_input.pdf", THREE_PAGE_TARGET_FIXTURE);
        let output = prepare_output(dir.path(), "ext_dup_output.pdf");

        extract_pages_pdf(ExtractPagesRequest {
            input_path: input,
            output_path: output.clone(),
            pages_to_extract: vec![2, 2],
        })
        .unwrap();

        let doc = Document::load(&output).unwrap();
        assert_eq!(
            doc.get_pages().len(),
            1,
            "Extracting page 2 with duplicates must yield 1 page"
        );
    }

    #[test]
    fn test_single_annotation_contents_normalization() {
        let dir = TempDir::new().unwrap();
        let input = write_fixture(
            dir.path(),
            "rotated_contents.pdf",
            ROTATED_CONTENTS_ARRAY_FIXTURE,
        );
        let output = prepare_output(dir.path(), "annotated_normalized.pdf");

        add_text_to_page(AddTextRequest {
            input_path: input,
            output_path: output.clone(),
            text: "Normalized Content Test".to_string(),
            page: 1,
            x: 50.0,
            y: 50.0,
            font_size: 14.0,
            color: "#FF0000".to_string(),
        })
        .unwrap();

        let doc = Document::load(&output).unwrap();
        let pages = doc.get_pages();
        let page_obj = doc.get_object(pages[&1]).unwrap();
        let contents = page_obj.as_dict().unwrap().get(b"Contents").unwrap();

        let arr = match contents {
            Object::Array(ref a) => a.clone(),
            Object::Reference(id) => match doc.get_object(*id).unwrap() {
                Object::Array(ref a) => a.clone(),
                other => panic!("Expected array object, got {:?}", other),
            },
            other => panic!("Expected Contents array, got {:?}", other),
        };

        for item in arr {
            let ref_id: ObjectId = item
                .as_reference()
                .expect("Each item in Contents must be a Reference");
            let target_obj = doc
                .get_object(ref_id)
                .expect("Referenced object must exist");
            assert!(
                matches!(target_obj, Object::Stream(_)),
                "Each Contents entry must reference a Stream, not an Array: {:?}",
                target_obj
            );
        }
    }
}
