use pdf_seeker_lib::commands::annotate::{
    add_highlight, add_page_numbers, add_rectangle, add_text_to_page, add_text_watermark,
    add_whiteout, AddHighlightRequest, AddPageNumbersRequest, AddRectangleRequest, AddTextRequest,
    WatermarkRequest, WhiteoutRequest,
};
use pdf_seeker_lib::commands::convert::extract_text;
use pdf_seeker_lib::commands::info::{compress_pdf, get_pdf_info, sanitize_pdf};
use pdf_seeker_lib::commands::organize::{
    delete_pages, extract_pages_pdf, merge_pdfs, reorder_pages, rotate_pdf, split_pdf,
    DeletePagesRequest, ExtractPagesRequest, ReorderPagesRequest, RotatePdfRequest,
    SplitPdfRequest,
};
use pdf_seeker_lib::pdf::io::load_doc;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

const USER_FILE_PATH: &str = "/Users/admin/Downloads/代码安全审计 - 实现文档-参考.pdf";

#[test]
fn test_user_document_full_functional_pipeline() {
    if !Path::new(USER_FILE_PATH).exists() {
        eprintln!(
            "User test file not found at {}, skipping live test",
            USER_FILE_PATH
        );
        return;
    }

    let temp = TempDir::new().unwrap();
    let temp_dir = temp.path();

    // Copy user file to work dir to avoid modifying original
    let work_input = temp_dir.join("original_work.pdf");
    fs::copy(USER_FILE_PATH, &work_input).expect("Failed to copy user test file");
    let input_path = work_input.to_string_lossy().to_string();

    println!("=== 1. Inspecting User Document Metadata ===");
    let info = get_pdf_info(input_path.clone()).expect("Failed to get PDF info");
    let total_pages = info.pages as usize;
    println!("Document Title: {:?}", info.title);
    println!("Total Pages: {}", total_pages);
    println!("File Size: {} bytes", info.file_size);
    println!("Image Count: {} (Vector document)", info.image_count);
    assert_eq!(
        info.image_count, 0,
        "User test file is pure vector text with 0 bitmap images"
    );
    assert!(total_pages > 0, "Document must have at least 1 page");

    // Test text extraction
    println!("=== 2. Testing Text Extraction ===");
    let text = extract_text(input_path.clone()).expect("Failed to extract text");
    println!("Extracted text length: {} chars", text.text.len());
    println!(
        "Preview of first 120 chars: {}",
        text.text.chars().take(120).collect::<String>()
    );
    assert!(!text.text.is_empty(), "Extracted text should not be empty");

    // Test sanitization (Validates Stirling-PDF hierarchical page tree fix)
    println!("=== 3. Testing PDF Sanitization & Page Tree Validation ===");
    let sanitized_out = temp_dir.join("sanitized.pdf").to_string_lossy().to_string();
    sanitize_pdf(input_path.clone(), sanitized_out.clone()).expect("Sanitize PDF failed");
    let sanitized_doc = load_doc(&sanitized_out).expect("Failed to reload sanitized PDF");
    assert_eq!(sanitized_doc.get_pages().len(), total_pages);
    println!(
        "Sanitization passed: validated {} pages successfully",
        total_pages
    );

    // Test compression
    println!("=== 4. Testing PDF Compression ===");
    let compressed_out = temp_dir
        .join("compressed.pdf")
        .to_string_lossy()
        .to_string();
    compress_pdf(input_path.clone(), compressed_out.clone()).expect("Compress PDF failed");
    let compressed_doc = load_doc(&compressed_out).expect("Failed to reload compressed PDF");
    assert_eq!(compressed_doc.get_pages().len(), total_pages);
    println!(
        "Compression passed: validated {} pages successfully",
        total_pages
    );

    // Test positive and negative rotation
    println!("=== 5. Testing Rotation (Positive & Negative Modulo) ===");
    let rotated_cw = temp_dir
        .join("rotated_cw.pdf")
        .to_string_lossy()
        .to_string();
    rotate_pdf(RotatePdfRequest {
        input_path: input_path.clone(),
        output_path: rotated_cw.clone(),
        angle: 90,
    })
    .expect("Rotate 90 deg failed");
    let doc_cw = load_doc(&rotated_cw).expect("Reload CW rotated PDF failed");
    assert_eq!(doc_cw.get_pages().len(), total_pages);

    let rotated_ccw = temp_dir
        .join("rotated_ccw.pdf")
        .to_string_lossy()
        .to_string();
    rotate_pdf(RotatePdfRequest {
        input_path: input_path.clone(),
        output_path: rotated_ccw.clone(),
        angle: -90,
    })
    .expect("Rotate -90 deg failed");
    let doc_ccw = load_doc(&rotated_ccw).expect("Reload CCW rotated PDF failed");
    assert_eq!(doc_ccw.get_pages().len(), total_pages);

    // Test watermark
    println!("=== 6. Testing Watermarking ===");
    let watermarked_out = temp_dir
        .join("watermarked.pdf")
        .to_string_lossy()
        .to_string();
    add_text_watermark(WatermarkRequest {
        input_path: input_path.clone(),
        output_path: watermarked_out.clone(),
        text: "交付校验 - CONFIDENTIAL".to_string(),
        font_size: 36.0,
        opacity: 0.3,
        angle: 45.0,
        color: "#FF0000".to_string(),
    })
    .expect("Watermark failed");
    let doc_wm = load_doc(&watermarked_out).expect("Reload watermarked PDF failed");
    assert_eq!(doc_wm.get_pages().len(), total_pages);

    // Test page numbering
    println!("=== 7. Testing Page Numbering ===");
    let numbered_out = temp_dir.join("numbered.pdf").to_string_lossy().to_string();
    add_page_numbers(AddPageNumbersRequest {
        input_path: input_path.clone(),
        output_path: numbered_out.clone(),
        format: "Page {n} of {total}".to_string(),
        position: "bottom-center".to_string(),
        start_page: Some(1),
        start_number: Some(1),
        font_size: Some(10.0),
        margin: Some(20.0),
        color: None,
    })
    .expect("Page numbering failed");
    let doc_num = load_doc(&numbered_out).expect("Reload numbered PDF failed");
    assert_eq!(doc_num.get_pages().len(), total_pages);

    // Test annotations (text, rectangle, highlight, whiteout)
    println!("=== 8. Testing Annotations & Content Normalization ===");
    let annotated_text = temp_dir
        .join("annotated_text.pdf")
        .to_string_lossy()
        .to_string();
    add_text_to_page(AddTextRequest {
        input_path: input_path.clone(),
        output_path: annotated_text.clone(),
        page: 1,
        text: "Audit Note: Verified".to_string(),
        x: 50.0,
        y: 50.0,
        font_size: 14.0,
        color: "#003366".to_string(),
    })
    .expect("Add text annotation failed");

    let annotated_rect = temp_dir
        .join("annotated_rect.pdf")
        .to_string_lossy()
        .to_string();
    add_rectangle(AddRectangleRequest {
        input_path: annotated_text.clone(),
        output_path: annotated_rect.clone(),
        page: 1,
        x: 40.0,
        y: 40.0,
        width: 150.0,
        height: 30.0,
        border_color: "#FF0000".to_string(),
        border_width: 2.0,
        fill_color: None,
    })
    .expect("Add rectangle failed");

    let annotated_hl = temp_dir
        .join("annotated_hl.pdf")
        .to_string_lossy()
        .to_string();
    add_highlight(AddHighlightRequest {
        input_path: annotated_rect.clone(),
        output_path: annotated_hl.clone(),
        page: 1,
        x: 40.0,
        y: 80.0,
        width: 120.0,
        height: 20.0,
        color: "#FFFF00".to_string(),
        opacity: 0.4,
    })
    .expect("Add highlight failed");

    let annotated_wo = temp_dir
        .join("annotated_wo.pdf")
        .to_string_lossy()
        .to_string();
    add_whiteout(WhiteoutRequest {
        input_path: annotated_hl.clone(),
        output_path: annotated_wo.clone(),
        page: 1,
        x: 10.0,
        y: 10.0,
        width: 30.0,
        height: 30.0,
    })
    .expect("Add whiteout failed");
    let doc_annotated = load_doc(&annotated_wo).expect("Reload annotated PDF failed");
    assert_eq!(doc_annotated.get_pages().len(), total_pages);

    // Test split & merge if multiple pages
    if total_pages >= 2 {
        println!("=== 9. Testing Split & Merge ===");
        let split_dir = temp_dir.join("split_pages");
        fs::create_dir_all(&split_dir).unwrap();
        let split_results = split_pdf(SplitPdfRequest {
            input_path: input_path.clone(),
            output_dir: split_dir.to_string_lossy().to_string(),
            mode: "all".to_string(),
            ranges: None,
        })
        .expect("Split PDF failed");
        assert_eq!(split_results.len(), total_pages);

        // Merge two split pages
        let merge_out = temp_dir
            .join("merged_2pages.pdf")
            .to_string_lossy()
            .to_string();
        merge_pdfs(
            vec![split_results[0].clone(), split_results[1].clone()],
            merge_out.clone(),
        )
        .expect("Merge PDFs failed");
        let doc_merged = load_doc(&merge_out).expect("Reload merged PDF failed");
        assert_eq!(doc_merged.get_pages().len(), 2);

        // Test delete pages with deduplication
        println!("=== 10. Testing Delete Pages (with duplicate arguments) ===");
        let delete_out = temp_dir
            .join("deleted_pages.pdf")
            .to_string_lossy()
            .to_string();
        delete_pages(DeletePagesRequest {
            input_path: input_path.clone(),
            output_path: delete_out.clone(),
            pages_to_delete: vec![1, 1], // Testing dedup fix
        })
        .expect("Delete pages failed");
        let doc_deleted = load_doc(&delete_out).expect("Reload deleted PDF failed");
        assert_eq!(doc_deleted.get_pages().len(), total_pages - 1);

        // Test extract pages with deduplication
        println!("=== 11. Testing Extract Pages (with duplicate arguments) ===");
        let extract_out = temp_dir
            .join("extracted_pages.pdf")
            .to_string_lossy()
            .to_string();
        extract_pages_pdf(ExtractPagesRequest {
            input_path: input_path.clone(),
            output_path: extract_out.clone(),
            pages_to_extract: vec![1, 1], // Testing dedup fix
        })
        .expect("Extract pages failed");
        let doc_extracted = load_doc(&extract_out).expect("Reload extracted PDF failed");
        assert_eq!(doc_extracted.get_pages().len(), 1);

        // Test reorder pages
        println!("=== 12. Testing Reorder Pages ===");
        let mut reordered_order: Vec<u32> = (1..=(total_pages as u32)).collect();
        reordered_order.reverse();
        let reorder_out = temp_dir.join("reordered.pdf").to_string_lossy().to_string();
        reorder_pages(ReorderPagesRequest {
            input_path: input_path.clone(),
            output_path: reorder_out.clone(),
            new_order: reordered_order,
        })
        .expect("Reorder pages failed");
        let doc_reordered = load_doc(&reorder_out).expect("Reload reordered PDF failed");
        assert_eq!(doc_reordered.get_pages().len(), total_pages);
    }

    // Safety check: In-place overwrite rejection
    println!("=== 13. Safety Boundary: In-place Overwrite Rejection ===");
    let overwrite_attempt = rotate_pdf(RotatePdfRequest {
        input_path: input_path.clone(),
        output_path: input_path.clone(),
        angle: 90,
    });
    assert!(
        overwrite_attempt.is_err(),
        "In-place overwrite must be rejected"
    );

    println!("=== All Real Document Verifications Passed Successfully! ===");
}
