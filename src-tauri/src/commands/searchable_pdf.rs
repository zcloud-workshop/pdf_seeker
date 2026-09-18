use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};

use crate::commands::ocr::OcrTextBox;
use crate::commands::pdf_content::{append_content_stream, ensure_resource_entry, get_or_create_resources};
use crate::commands::validation::{validate_output_path, validate_path};
use crate::error::{AppError, AppResult};
use crate::pdf::io::{self, ensure_distinct_output, ValidationPolicy};
use crate::pdf::text_postprocess::clean_ocr_text;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PageOcrData {
    pub page: u32,
    pub image_width: f64,
    pub image_height: f64,
    pub boxes: Vec<OcrTextBox>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSearchablePdfRequest {
    pub input_path: String,
    pub output_path: String,
    pub pages_data: Vec<PageOcrData>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchablePdfResult {
    pub output_path: String,
    pub total_pages_processed: usize,
    pub total_boxes_injected: usize,
}

/// Convert text into UTF-16BE hex string format `<00410042...>` for Identity-H ToUnicode font
fn text_to_utf16be_hex(text: &str) -> String {
    let mut hex = String::with_capacity(text.len() * 4 + 2);
    hex.push('<');
    for unit in text.encode_utf16() {
        use std::fmt::Write;
        let _ = write!(hex, "{:04X}", unit);
    }
    hex.push('>');
    hex
}


fn extract_num(obj: &Object, default: f64) -> f64 {
    match obj {
        Object::Real(f) => *f as f64,
        Object::Integer(i) => *i as f64,
        _ => default,
    }
}

fn get_page_dimensions(doc: &Document, page_id: &ObjectId) -> (f64, f64, f64, f64) {
    if let Ok(page_obj) = doc.get_object(*page_id) {
        if let Ok(dict) = page_obj.as_dict() {
            if let Ok(media_box) = dict.get(b"MediaBox").and_then(|o| o.as_array()) {
                if media_box.len() >= 4 {
                    let x0 = extract_num(&media_box[0], 0.0);
                    let y0 = extract_num(&media_box[1], 0.0);
                    let x1 = extract_num(&media_box[2], 595.0);
                    let y1 = extract_num(&media_box[3], 842.0);
                    return (x0, y0, (x1 - x0).abs(), (y1 - y0).abs());
                }
            }
        }
    }
    (0.0, 0.0, 595.0, 842.0)
}

/// Creates a dual-layer Searchable PDF (Sandwich PDF) by injecting an invisible
/// text stream (mode 3 Tr) positioned exactly over the scanned/bitmap document.
#[tauri::command]
pub fn create_searchable_pdf(req: CreateSearchablePdfRequest) -> AppResult<SearchablePdfResult> {
    validate_path(&req.input_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(&req.output_path, &[req.input_path.as_str()])?;

    let mut doc = io::load_doc(&req.input_path)?;
    io::reject_encrypted(&doc, &req.input_path)?;
    let pages = doc.get_pages();
    let total_pages = pages.len();

    if req.pages_data.is_empty() {
        return Err(AppError::Pdf("No OCR page data provided".into()));
    }

    // Register a ToUnicode CMap stream and Type0 font for high-fidelity multi-language search/copy
    let cmap_data = b"/CIDInit /ProcSet findresource begin\n\
12 dict begin\n\
begincmap\n\
/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
/CMapName /Custom-ToUnicode def\n\
/CMapType 2 def\n\
1 begincodespacerange\n\
<0000> <ffff>\n\
endcodespacerange\n\
1 beginbfrange\n\
<0000> <ffff> <0000>\n\
endbfrange\n\
endcmap\n\
CMapName currentdict /CMap defineresource pop\n\
end\n\
end\n";

    let tounicode_id = doc.add_object(Stream::new(dictionary! {}, cmap_data.to_vec()));
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "Helvetica",
        "Encoding" => "Identity-H",
        "ToUnicode" => Object::Reference(tounicode_id),
    });

    let mut total_boxes_injected = 0;
    let mut total_pages_processed = 0;

    for page_data in &req.pages_data {
        let page_id = match pages.get(&page_data.page) {
            Some(id) => *id,
            None => continue,
        };

        if page_data.image_width <= 0.0 || page_data.image_height <= 0.0 {
            continue;
        }

        let (x0, y0, page_w, page_h) = get_page_dimensions(&doc, &page_id);
        let scale_x = page_w / page_data.image_width;
        let scale_y = page_h / page_data.image_height;

        // Ensure page resources has font reference
        let (needs_res_update, res_id) = get_or_create_resources(&mut doc, &page_id)?;
        ensure_resource_entry(&mut doc, res_id, "Font", b"F_OCR", Object::Reference(font_id))?;
        if needs_res_update {
            if let Ok(p_obj) = doc.get_object_mut(page_id) {
                if let Object::Dictionary(ref mut p_dict) = p_obj {
                    p_dict.set("Resources", Object::Reference(res_id));
                }
            }
        }

        // Build invisible text content stream
        let mut stream_data = String::with_capacity(page_data.boxes.len() * 128);
        stream_data.push_str("q\n");
        stream_data.push_str("3 Tr\n"); // Mode 3: Neither fill nor stroke text (invisible!)

        let mut page_box_count = 0;

        for b in &page_data.boxes {
            let text = b.text.trim();
            if text.is_empty() || b.points.len() < 4 {
                continue;
            }

            let p_tl = &b.points[0];
            let _p_tr = &b.points[1];
            let p_bl = &b.points[3];

            let box_x = p_tl[0] as f64 * scale_x;
            let box_top = p_tl[1] as f64 * scale_y;
            let box_bottom = p_bl[1] as f64 * scale_y;
            let box_height = (box_bottom - box_top).abs();

            let pdf_x = x0 + box_x;
            let pdf_y = y0 + page_h - box_bottom; // baseline near bottom of box
            let font_size = (box_height * 0.85).clamp(4.0, 72.0);

            stream_data.push_str("BT\n");
            stream_data.push_str(&format!("/F_OCR {:.2} Tf\n", font_size));
            stream_data.push_str(&format!("1 0 0 1 {:.2} {:.2} Tm\n", pdf_x, pdf_y));
            stream_data.push_str(&format!("{} Tj\n", text_to_utf16be_hex(text)));
            stream_data.push_str("ET\n");
            page_box_count += 1;
        }

        stream_data.push_str("Q\n");

        if page_box_count > 0 {
            let content_obj_id = doc.add_object(Stream::new(dictionary! {}, stream_data.into_bytes()));
            append_content_stream(&mut doc, &page_id, content_obj_id)?;
            total_boxes_injected += page_box_count;
            total_pages_processed += 1;
        }
    }

    let policy = ValidationPolicy {
        expected_pages: Some(total_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;

    Ok(SearchablePdfResult {
        output_path: req.output_path,
        total_pages_processed,
        total_boxes_injected,
    })
}

/// Applies text cleaning pipeline (hyphen removal, paragraph merging, CJK spacing)
#[tauri::command]
pub fn postprocess_text(raw_text: String) -> String {
    clean_ocr_text(&raw_text)
}
