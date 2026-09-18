use std::fs;
use std::path::Path;
use tempfile::TempDir;

use pdf_seeker_lib::commands::convert::extract_text;
use pdf_seeker_lib::commands::ocr::OcrTextBox;
use pdf_seeker_lib::commands::searchable_pdf::{
    create_searchable_pdf, postprocess_text, CreateSearchablePdfRequest, PageOcrData,
};
use pdf_seeker_lib::pdf::io::load_doc;

const SINGLE_PAGE_FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/single-page-text.pdf"
));

fn write_fixture(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path.to_string_lossy().to_string()
}

fn prepare_output(dir: &Path, name: &str) -> String {
    dir.join(name).to_string_lossy().to_string()
}

#[test]
fn test_create_searchable_pdf_injects_invisible_text() {
    let temp = TempDir::new().unwrap();
    let input = write_fixture(temp.path(), "input.pdf", SINGLE_PAGE_FIXTURE);
    let output = prepare_output(temp.path(), "searchable.pdf");

    let ocr_box1 = OcrTextBox {
        points: vec![[50.0, 100.0], [250.0, 100.0], [250.0, 130.0], [50.0, 130.0]],
        text: "Searchable OCR Header".to_string(),
        confidence: 0.98,
    };

    let ocr_box2 = OcrTextBox {
        points: vec![[50.0, 150.0], [300.0, 150.0], [300.0, 180.0], [50.0, 180.0]],
        text: "机密扫描文档 2026".to_string(),
        confidence: 0.95,
    };

    let page_data = PageOcrData {
        page: 1,
        image_width: 800.0,
        image_height: 1000.0,
        boxes: vec![ocr_box1, ocr_box2],
    };

    let req = CreateSearchablePdfRequest {
        input_path: input.clone(),
        output_path: output.clone(),
        pages_data: vec![page_data],
    };

    let res = create_searchable_pdf(req).expect("create_searchable_pdf should succeed");
    assert_eq!(res.total_pages_processed, 1);
    assert_eq!(res.total_boxes_injected, 2);

    // Verify document opens and has correct structure
    let doc = load_doc(&output).expect("Output must reopen cleanly");
    assert_eq!(doc.get_pages().len(), 1);

    // Verify the injected invisible text is extractable
    let extracted = extract_text(output).expect("extract_text must succeed");
    println!("EXTRACTED TEXT: {:?}", extracted.text);
    assert!(
        extracted.text.contains("Searchable OCR Header"),
        "Extracted text should contain injected Latin text"
    );
    assert!(
        extracted.text.contains("机密扫描文档 2026"),
        "Extracted text should contain injected Unicode CJK text"
    );
}

#[test]
fn test_searchable_pdf_in_place_overwrite_rejected() {
    let temp = TempDir::new().unwrap();
    let path = write_fixture(temp.path(), "input.pdf", SINGLE_PAGE_FIXTURE);

    let req = CreateSearchablePdfRequest {
        input_path: path.clone(),
        output_path: path.clone(),
        pages_data: vec![PageOcrData {
            page: 1,
            image_width: 500.0,
            image_height: 500.0,
            boxes: vec![],
        }],
    };

    let res = create_searchable_pdf(req);
    assert!(
        res.is_err(),
        "Identical input and output path must be rejected"
    );
}

#[test]
fn test_postprocess_text_command() {
    let raw = "The inter-\n   national treaty on PDF\ntechnology was signed.";
    let cleaned = postprocess_text(raw.to_string());
    assert!(cleaned.contains("international treaty on PDF technology was signed."));

    let mixed = "PDF阅读器2026版本";
    assert_eq!(postprocess_text(mixed.to_string()), "PDF 阅读器 2026 版本");
}
