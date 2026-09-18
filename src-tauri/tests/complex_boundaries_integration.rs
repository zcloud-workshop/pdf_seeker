use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use tempfile::TempDir;
use pdf_seeker_lib::commands::annotate::{
    add_page_numbers, add_text_watermark, AddPageNumbersRequest, WatermarkRequest,
};
use pdf_seeker_lib::commands::convert::extract_text;
use pdf_seeker_lib::commands::info::{compress_pdf, sanitize_pdf};
use pdf_seeker_lib::commands::organize::{
    delete_pages, extract_pages_pdf, insert_pages, merge_pdfs, reorder_pages, rotate_pdf,
    split_pdf, DeletePagesRequest, ExtractPagesRequest, InsertPagesRequest, ReorderPagesRequest,
    RotatePdfRequest, SplitPdfRequest,
};
use pdf_seeker_lib::pdf::io::load_doc;

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
const CORRUPTED_PDF_FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/truncated-input.pdf"
));
const ENCRYPTED_PDF_FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/encrypted-test.pdf"
));
const CHINESE_TEXT_FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/chinese-text.pdf"
));

fn write_fixture(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path.to_string_lossy().to_string()
}

fn prepare_output(dir: &Path, name: &str) -> String {
    dir.join(name).to_string_lossy().to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 1: Complex Multi-Stage Pipeline Integrity
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn test_complex_multi_stage_pipeline() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path();

    // Fixtures
    let target_path = write_fixture(temp_path, "target.pdf", THREE_PAGE_TARGET_FIXTURE);
    let source_path = write_fixture(temp_path, "source.pdf", TWO_PAGE_SOURCE_FIXTURE);
    let single_path = write_fixture(temp_path, "single.pdf", SINGLE_PAGE_FIXTURE);

    // Stage 1: Merge 3-page target and 2-page source -> 5-page composite
    let stage1_output = prepare_output(temp_path, "stage1_merged.pdf");
    merge_pdfs(vec![target_path, source_path], stage1_output.clone()).unwrap();
    assert_eq!(load_doc(&stage1_output).unwrap().get_pages().len(), 5);

    // Stage 2: Rotate page 1 by 90 degrees
    let stage2_output = prepare_output(temp_path, "stage2_rotated.pdf");
    rotate_pdf(RotatePdfRequest {
        input_path: stage1_output.clone(),
        output_path: stage2_output.clone(),
        angle: 90,
    })
    .unwrap();
    assert_eq!(load_doc(&stage2_output).unwrap().get_pages().len(), 5);

    // Stage 3: Insert single page at position 2 -> 6 pages
    let stage3_output = prepare_output(temp_path, "stage3_inserted.pdf");
    insert_pages(InsertPagesRequest {
        input_path: stage2_output.clone(),
        source_path: single_path,
        insert_position: 2,
        output_path: stage3_output.clone(),
    })
    .unwrap();
    assert_eq!(load_doc(&stage3_output).unwrap().get_pages().len(), 6);

    // Stage 4: Reorder pages in reverse [6, 5, 4, 3, 2, 1]
    let stage4_output = prepare_output(temp_path, "stage4_reordered.pdf");
    reorder_pages(ReorderPagesRequest {
        input_path: stage3_output.clone(),
        output_path: stage4_output.clone(),
        new_order: vec![6, 5, 4, 3, 2, 1],
    })
    .unwrap();
    assert_eq!(load_doc(&stage4_output).unwrap().get_pages().len(), 6);

    // Stage 5: Add page numbers with template
    let stage5_output = prepare_output(temp_path, "stage5_numbered.pdf");
    add_page_numbers(AddPageNumbersRequest {
        input_path: stage4_output.clone(),
        output_path: stage5_output.clone(),
        format: "Page {n} of {total}".to_string(),
        position: "bottom-center".to_string(),
        start_page: Some(1),
        start_number: Some(1),
        font_size: Some(10.0),
        color: Some("#333333".to_string()),
        margin: None,
    })
    .unwrap();
    assert_eq!(load_doc(&stage5_output).unwrap().get_pages().len(), 6);

    // Stage 6: Lossless compression
    let stage6_output = prepare_output(temp_path, "stage6_compressed.pdf");
    let compress_res = compress_pdf(stage5_output.clone(), stage6_output.clone()).unwrap();
    assert!(compress_res.compressed_size > 0);
    assert_eq!(load_doc(&stage6_output).unwrap().get_pages().len(), 6);

    // Stage 7: Metadata sanitization (strip Info and XMP)
    let stage7_output = prepare_output(temp_path, "stage7_sanitized.pdf");
    sanitize_pdf(stage6_output.clone(), stage7_output.clone()).unwrap();
    let doc7 = load_doc(&stage7_output).unwrap();
    assert_eq!(doc7.get_pages().len(), 6);
    assert!(doc7.trailer.get(b"Info").is_err());

    // Stage 8: Split into single pages
    let split_dir = temp_path.join("split_pages");
    fs::create_dir_all(&split_dir).unwrap();
    let split_res = split_pdf(SplitPdfRequest {
        input_path: stage7_output,
        output_dir: split_dir.to_string_lossy().to_string(),
        mode: "all".to_string(),
        ranges: None,
    })
    .unwrap();
    assert_eq!(split_res.len(), 6);

    // Verify all 6 split files are valid single-page PDFs
    for path_str in split_res {
        let single_doc = load_doc(&path_str).unwrap();
        assert_eq!(single_doc.get_pages().len(), 1);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 2: Extreme and Out-of-Bounds Page Handling
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn test_extreme_and_out_of_bounds_pages() {
    let temp = TempDir::new().unwrap();
    let target_path = write_fixture(temp.path(), "three_pages.pdf", THREE_PAGE_TARGET_FIXTURE);

    // 1. Delete all pages (must fail to prevent creating 0-page document)
    let out1 = prepare_output(temp.path(), "delete_all.pdf");
    let res = delete_pages(DeletePagesRequest {
        input_path: target_path.clone(),
        output_path: out1.clone(),
        pages_to_delete: vec![1, 2, 3],
    });
    assert!(res.is_err(), "Deleting all pages should return error");
    assert!(!Path::new(&out1).exists());

    // 2. Delete out-of-bounds page (page 99 on a 3-page doc)
    let out2 = prepare_output(temp.path(), "delete_oob.pdf");
    let res = delete_pages(DeletePagesRequest {
        input_path: target_path.clone(),
        output_path: out2.clone(),
        pages_to_delete: vec![99],
    });
    assert!(res.is_err(), "Deleting out-of-bounds page should return error");
    assert!(!Path::new(&out2).exists());

    // 3. Delete with empty page list
    let out3 = prepare_output(temp.path(), "delete_empty.pdf");
    let res = delete_pages(DeletePagesRequest {
        input_path: target_path.clone(),
        output_path: out3.clone(),
        pages_to_delete: vec![],
    });
    assert!(res.is_err(), "Deleting empty page list should return error");
    assert!(!Path::new(&out3).exists());

    // 4. Extract out-of-bounds page
    let out4 = prepare_output(temp.path(), "extract_oob.pdf");
    let res = extract_pages_pdf(ExtractPagesRequest {
        input_path: target_path.clone(),
        output_path: out4.clone(),
        pages_to_extract: vec![99],
    });
    assert!(res.is_err(), "Extracting out-of-bounds page should fail");
    assert!(!Path::new(&out4).exists());

    // 5. Extract empty page list
    let out5 = prepare_output(temp.path(), "extract_empty.pdf");
    let res = extract_pages_pdf(ExtractPagesRequest {
        input_path: target_path.clone(),
        output_path: out5.clone(),
        pages_to_extract: vec![],
    });
    assert!(res.is_err(), "Extracting empty list should fail");
    assert!(!Path::new(&out5).exists());

    // 6. Reorder with out-of-bounds page index
    let out6 = prepare_output(temp.path(), "reorder_oob.pdf");
    let res = reorder_pages(ReorderPagesRequest {
        input_path: target_path.clone(),
        output_path: out6.clone(),
        new_order: vec![1, 2, 99],
    });
    assert!(res.is_err(), "Reorder with out-of-bounds index should fail");
    assert!(!Path::new(&out6).exists());

    // 7. Reorder with count mismatch (less pages than document)
    let out7 = prepare_output(temp.path(), "reorder_mismatch.pdf");
    let res = reorder_pages(ReorderPagesRequest {
        input_path: target_path.clone(),
        output_path: out7.clone(),
        new_order: vec![1, 2],
    });
    assert!(res.is_err(), "Reorder with missing pages should fail");
    assert!(!Path::new(&out7).exists());

    // 8. Insert at invalid page index (> total_pages)
    let single_path = write_fixture(temp.path(), "single.pdf", SINGLE_PAGE_FIXTURE);
    let out8 = prepare_output(temp.path(), "insert_oob.pdf");
    let res = insert_pages(InsertPagesRequest {
        input_path: target_path.clone(),
        source_path: single_path,
        insert_position: 100,
        output_path: out8.clone(),
    });
    assert!(res.is_err(), "Insert at page > total should fail");
    assert!(!Path::new(&out8).exists());
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 3: Adversarial and Malformed Input Robustness
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn test_adversarial_and_malformed_input_robustness() {
    let temp = TempDir::new().unwrap();
    let corrupt_path = write_fixture(temp.path(), "truncated.pdf", CORRUPTED_PDF_FIXTURE);
    let valid_path = write_fixture(temp.path(), "valid.pdf", SINGLE_PAGE_FIXTURE);
    let encrypted_path = write_fixture(temp.path(), "encrypted.pdf", ENCRYPTED_PDF_FIXTURE);

    // Merge with corrupted file must fail cleanly
    let out_merge = prepare_output(temp.path(), "corrupt_merge.pdf");
    assert!(merge_pdfs(vec![corrupt_path.clone(), valid_path.clone()], out_merge.clone()).is_err());
    assert!(!Path::new(&out_merge).exists());

    // Rotate corrupted file must fail
    let out_rot = prepare_output(temp.path(), "corrupt_rotate.pdf");
    assert!(rotate_pdf(RotatePdfRequest {
        input_path: corrupt_path.clone(),
        output_path: out_rot.clone(),
        angle: 90,
    })
    .is_err());
    assert!(!Path::new(&out_rot).exists());

    // Extract text from corrupted file must fail
    assert!(extract_text(corrupt_path.clone()).is_err());

    // Compress corrupted file must fail
    let out_comp = prepare_output(temp.path(), "corrupt_compress.pdf");
    assert!(compress_pdf(corrupt_path.clone(), out_comp.clone()).is_err());
    assert!(!Path::new(&out_comp).exists());

    // Sanitize corrupted file must fail
    let out_san = prepare_output(temp.path(), "corrupt_sanitize.pdf");
    assert!(sanitize_pdf(corrupt_path.clone(), out_san.clone()).is_err());
    assert!(!Path::new(&out_san).exists());

    // Add page numbers to corrupted file must fail
    let out_num = prepare_output(temp.path(), "corrupt_number.pdf");
    assert!(add_page_numbers(AddPageNumbersRequest {
        input_path: corrupt_path.clone(),
        output_path: out_num.clone(),
        format: "{n}".into(),
        position: "bottom-center".into(),
        start_page: Some(1),
        start_number: Some(1),
        font_size: Some(12.0),
        color: Some("#000000".into()),
        margin: None,
    })
    .is_err());
    assert!(!Path::new(&out_num).exists());

    // Encrypted file operations must be detected and rejected
    let out_enc_rot = prepare_output(temp.path(), "enc_rotate.pdf");
    assert!(rotate_pdf(RotatePdfRequest {
        input_path: encrypted_path.clone(),
        output_path: out_enc_rot.clone(),
        angle: 90,
    })
    .is_err());
    assert!(!Path::new(&out_enc_rot).exists());

    let out_enc_san = prepare_output(temp.path(), "enc_sanitize.pdf");
    assert!(sanitize_pdf(encrypted_path.clone(), out_enc_san.clone()).is_err());
    assert!(!Path::new(&out_enc_san).exists());
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 4: Special Characters and CJK Injection Defense
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn test_special_characters_and_cjk_injection() {
    let temp = TempDir::new().unwrap();
    let chinese_path = write_fixture(temp.path(), "chinese.pdf", CHINESE_TEXT_FIXTURE);

    // 1. Watermark with PDF escape characters: (, ), \, /, %, \n, \r
    let out_escape = prepare_output(temp.path(), "escape_watermark.pdf");
    let escape_text = "Watermark (Special) \\ Escaped / Path % Token \r\n Line2";
    let res = add_text_watermark(WatermarkRequest {
        input_path: chinese_path.clone(),
        output_path: out_escape.clone(),
        text: escape_text.to_string(),
        font_size: 14.0,
        opacity: 0.3,
        angle: 45.0,
        color: "#FF0000".to_string(),
    });
    assert!(res.is_ok(), "Watermark with escaped characters should succeed");
    let doc_esc = load_doc(&out_escape).unwrap();
    assert_eq!(doc_esc.get_pages().len(), 2);

    // 2. Watermark with CJK Unicode string
    let out_cjk = prepare_output(temp.path(), "cjk_watermark.pdf");
    let cjk_text = "机密档案 内部审阅 2026";
    let res_cjk = add_text_watermark(WatermarkRequest {
        input_path: chinese_path.clone(),
        output_path: out_cjk.clone(),
        text: cjk_text.to_string(),
        font_size: 16.0,
        opacity: 0.5,
        angle: 30.0,
        color: "#003366".to_string(),
    });
    assert!(res_cjk.is_ok(), "Watermark with CJK text should succeed");
    let doc_cjk = load_doc(&out_cjk).unwrap();
    assert_eq!(doc_cjk.get_pages().len(), 2);

    // 3. Add page numbers with custom template and special symbols
    let out_num = prepare_output(temp.path(), "symbol_numbering.pdf");
    let num_template = "« Page {n} / {total} (Confidential) »";
    let res_num = add_page_numbers(AddPageNumbersRequest {
        input_path: chinese_path,
        output_path: out_num.clone(),
        format: num_template.to_string(),
        position: "bottom-right".to_string(),
        start_page: Some(1),
        start_number: Some(1),
        font_size: Some(11.0),
        color: Some("#222222".to_string()),
        margin: None,
    });
    assert!(res_num.is_ok(), "Page numbers with symbols should succeed");
    let doc_num = load_doc(&out_num).unwrap();
    assert_eq!(doc_num.get_pages().len(), 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 5: In-Place Overwrite Prevention and Concurrency
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn test_in_place_overwrite_rejected() {
    let temp = TempDir::new().unwrap();
    let path = write_fixture(temp.path(), "single.pdf", SINGLE_PAGE_FIXTURE);

    // In-place overwrite for rotate_pdf should be rejected
    assert!(
        rotate_pdf(RotatePdfRequest {
            input_path: path.clone(),
            output_path: path.clone(),
            angle: 90,
        })
        .is_err(),
        "rotate_pdf with identical in/out paths must be rejected"
    );

    // In-place overwrite for compress_pdf should be rejected
    assert!(
        compress_pdf(path.clone(), path.clone()).is_err(),
        "compress_pdf with identical in/out paths must be rejected"
    );

    // In-place overwrite for sanitize_pdf should be rejected
    assert!(
        sanitize_pdf(path.clone(), path.clone()).is_err(),
        "sanitize_pdf with identical in/out paths must be rejected"
    );

    // In-place overwrite for add_page_numbers should be rejected
    assert!(
        add_page_numbers(AddPageNumbersRequest {
            input_path: path.clone(),
            output_path: path.clone(),
            format: "{n}".into(),
            position: "bottom-center".into(),
            start_page: Some(1),
            start_number: Some(1),
            font_size: Some(12.0),
            color: Some("#000000".into()),
            margin: None,
        })
        .is_err(),
        "add_page_numbers with identical in/out paths must be rejected"
    );
}

#[test]
fn test_multithreaded_concurrency_isolation() {
    let temp = Arc::new(TempDir::new().unwrap());
    let mut handles = Vec::new();

    // Spawn 6 threads concurrently performing various operations
    for i in 0..6 {
        let temp_ref = Arc::clone(&temp);
        let handle = thread::spawn(move || {
            let thread_dir = temp_ref.path().join(format!("thread_{}", i));
            fs::create_dir_all(&thread_dir).unwrap();

            let input_path = write_fixture(&thread_dir, "input.pdf", THREE_PAGE_TARGET_FIXTURE);
            let rotated_path = prepare_output(&thread_dir, "rotated.pdf");
            let numbered_path = prepare_output(&thread_dir, "numbered.pdf");
            let compressed_path = prepare_output(&thread_dir, "compressed.pdf");

            // Operation 1: Rotate
            rotate_pdf(RotatePdfRequest {
                input_path,
                output_path: rotated_path.clone(),
                angle: 90,
            })
            .unwrap();

            // Operation 2: Add page numbers
            add_page_numbers(AddPageNumbersRequest {
                input_path: rotated_path.clone(),
                output_path: numbered_path.clone(),
                format: format!("T{}-P{{n}}", i),
                position: "bottom-right".into(),
                start_page: Some(1),
                start_number: Some(1),
                font_size: Some(10.0),
                color: Some("#111111".into()),
                margin: None,
            })
            .unwrap();

            // Operation 3: Compress
            compress_pdf(numbered_path.clone(), compressed_path.clone()).unwrap();

            let final_doc = load_doc(&compressed_path).unwrap();
            assert_eq!(final_doc.get_pages().len(), 3);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Worker thread panicked");
    }
}
