use lopdf::{Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use image as img_crate;

pub type ObjId = ObjectId;
pub type AppResult<T> = Result<T, String>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotatePdfRequest {
    pub input_path: String,
    pub output_path: String,
    pub angle: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePagesRequest {
    pub input_path: String,
    pub output_path: String,
    pub pages_to_delete: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct TextExtractResult {
    pub text: String,
    pub pages: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitPdfRequest {
    pub input_path: String,
    pub output_dir: String,
    pub mode: String,
    pub ranges: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractPagesRequest {
    pub input_path: String,
    pub output_path: String,
    pub pages_to_extract: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct CompressResult {
    pub original_size: u64,
    pub compressed_size: u64,
    pub ratio: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatermarkRequest {
    pub input_path: String,
    pub output_path: String,
    pub text: String,
    pub font_size: f64,
    pub opacity: f64,
    pub angle: f64,
    pub color: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagesToPdfRequest {
    pub image_paths: Vec<String>,
    pub output_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderPagesRequest {
    pub input_path: String,
    pub output_path: String,
    pub new_order: Vec<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertPagesRequest {
    pub input_path: String,
    pub source_path: String,
    pub output_path: String,
    pub insert_position: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignPdfRequest {
    pub input_path: String,
    pub output_path: String,
    pub signature_image_path: String,
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrRequest {
    pub image_dir: String,
    pub language: String,
}

fn load_doc(path: &str) -> AppResult<Document> {
    Document::load(path).map_err(|e| format!("Load '{}': {}", path, e))
}

fn save_doc(doc: &mut Document, path: &str) -> AppResult<()> {
    doc.save(path)
        .map(|_| ())
        .map_err(|e| format!("Save '{}': {}", path, e))
}

fn escape_pdf_string(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('(', "\\(")
     .replace(')', "\\)")
}

fn parse_page_ranges(ranges: &str, max: u32) -> AppResult<Vec<Vec<u32>>> {
    if ranges.trim().is_empty() {
        return Err("Empty ranges".into());
    }
    let mut result = Vec::new();
    for part in ranges.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() { continue; }
        if trimmed.contains('-') {
            let nums: Vec<&str> = trimmed.split('-').collect();
            if nums.len() != 2 { return Err(format!("Invalid range: {}", trimmed)); }
            let s: u32 = nums[0].parse().map_err(|_| format!("Invalid number: {}", nums[0]))?;
            let e: u32 = nums[1].parse().map_err(|_| format!("Invalid number: {}", nums[1]))?;
            if s < 1 || e > max || s > e { return Err(format!("Range {} out of bounds (1-{})", trimmed, max)); }
            result.push((s..=e).collect());
        } else {
            let n: u32 = trimmed.parse().map_err(|_| format!("Invalid number: {}", trimmed))?;
            if n < 1 || n > max { return Err(format!("Page {} out of bounds (1-{})", n, max)); }
            result.push(vec![n]);
        }
    }
    if result.is_empty() { return Err("No valid ranges".into()); }
    Ok(result)
}

fn embed_image(doc: &mut Document, data: &[u8], path: &str) -> AppResult<(ObjectId, u32, u32)> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "jpg" || ext == "jpeg" {
        let img = img_crate::load_from_memory(data)
            .map_err(|e| format!("Image decode: {}", e))?;
        let (w, h) = (img.width(), img.height());
        let dict = lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"XObject".to_vec())),
            (b"Subtype".to_vec(), Object::Name(b"Image".to_vec())),
            (b"Width".to_vec(), Object::Integer(w as i64)),
            (b"Height".to_vec(), Object::Integer(h as i64)),
            (b"ColorSpace".to_vec(), Object::Name(b"DeviceRGB".to_vec())),
            (b"BitsPerComponent".to_vec(), Object::Integer(8)),
            (b"Filter".to_vec(), Object::Name(b"DCTDecode".to_vec())),
        ]);
        let id = doc.add_object(Object::Stream(lopdf::Stream::new(dict, data.to_vec())));
        Ok((id, w, h))
    } else {
        let img = img_crate::load_from_memory(data)
            .map_err(|e| format!("Image decode: {}", e))?;
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        let mut rgb_data = Vec::with_capacity((w * h * 3) as usize);
        let mut alpha_data = Vec::with_capacity((w * h) as usize);
        for px in rgba.pixels() {
            rgb_data.extend_from_slice(&[px[0], px[1], px[2]]);
            alpha_data.push(px[3]);
        }
        let dict = lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"XObject".to_vec())),
            (b"Subtype".to_vec(), Object::Name(b"Image".to_vec())),
            (b"Width".to_vec(), Object::Integer(w as i64)),
            (b"Height".to_vec(), Object::Integer(h as i64)),
            (b"ColorSpace".to_vec(), Object::Name(b"DeviceRGB".to_vec())),
            (b"BitsPerComponent".to_vec(), Object::Integer(8)),
            (b"Filter".to_vec(), Object::Name(b"FlateDecode".to_vec())),
        ]);
        let id = doc.add_object(Object::Stream(lopdf::Stream::new(dict, rgb_data)));
        let smask_dict = lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"XObject".to_vec())),
            (b"Subtype".to_vec(), Object::Name(b"Image".to_vec())),
            (b"Width".to_vec(), Object::Integer(w as i64)),
            (b"Height".to_vec(), Object::Integer(h as i64)),
            (b"ColorSpace".to_vec(), Object::Name(b"DeviceGray".to_vec())),
            (b"BitsPerComponent".to_vec(), Object::Integer(8)),
            (b"Filter".to_vec(), Object::Name(b"FlateDecode".to_vec())),
        ]);
        let smask_id = doc.add_object(Object::Stream(lopdf::Stream::new(smask_dict, alpha_data)));
        if let Some(obj) = doc.objects.get_mut(&id) {
            if let Ok(stream) = obj.as_stream_mut() {
                stream.dict.set(b"SMask", Object::Reference(smask_id));
            }
        }
        Ok((id, w, h))
    }
}

fn get_pages_ref(doc: &Document) -> AppResult<ObjectId> {
    let root_ref = doc.trailer.get(b"Root")
        .and_then(|o| o.as_reference())
        .map_err(|e| format!("Root error: {}", e))?;
    doc.get_object(root_ref)
        .and_then(|o| o.as_dict())
        .and_then(|d| d.get(b"Pages"))
        .and_then(|o| o.as_reference())
        .map_err(|e| format!("Pages error: {}", e))
}

fn get_page_size(page_dict: &lopdf::Dictionary) -> (f64, f64) {
    page_dict.get(b"MediaBox").ok()
        .and_then(|mb| mb.as_array().ok())
        .map(|arr| {
            let w = arr.get(2).and_then(|o| o.as_i64().ok()).unwrap_or(612) as f64;
            let h = arr.get(3).and_then(|o| o.as_i64().ok()).unwrap_or(792) as f64;
            (w, h)
        })
        .unwrap_or((612.0, 792.0))
}

#[tauri::command]
pub fn merge_pdfs(paths: Vec<String>, output_path: String) -> AppResult<()> {
    if paths.is_empty() {
        return Err("No input PDFs".into());
    }

    let mut merged = load_doc(&paths[0])?;

    for path in paths.iter().skip(1) {
        let mut doc = load_doc(path)?;

        // Collect page IDs and all object IDs BEFORE renumbering
        let old_page_ids: Vec<ObjId> = doc.get_pages().values().copied().collect();
        let mut sorted_old_ids: Vec<ObjId> = doc.objects.keys().copied().collect();
        sorted_old_ids.sort();

        // Renumber so IDs don't collide with merged's objects
        let start_id = merged.max_id + 1;
        doc.renumber_objects_with(start_id);

        // Build old→new ID mapping (sorted old IDs → sequential new IDs)
        let id_map: std::collections::BTreeMap<ObjId, ObjId> = sorted_old_ids
            .iter()
            .enumerate()
            .map(|(i, old)| (*old, (start_id + i as u32, 0)))
            .collect();

        // Map old page IDs to new IDs
        let doc_pages: Vec<ObjId> = old_page_ids
            .iter()
            .map(|old| *id_map.get(old).unwrap_or(old))
            .collect();

        for (id, obj) in doc.objects {
            merged.objects.insert(id, obj);
        }
        // Update max_id so save() includes all objects in the xref table
        if let Some(max_key) = merged.objects.keys().max() {
            merged.max_id = merged.max_id.max(max_key.0);
        }

        let root_ref = merged
            .trailer
            .get(b"Root")
            .and_then(|o| o.as_reference())
            .map_err(|e| format!("Root error: {}", e))?;

        let pages_ref = merged
            .get_object(root_ref)
            .and_then(|o| o.as_dict())
            .and_then(|d| d.get(b"Pages"))
            .and_then(|o| o.as_reference())
            .map_err(|e| format!("Pages error: {}", e))?;

        let merged_count = merged.get_pages().len();

        let pages_obj = merged
            .objects
            .get_mut(&pages_ref)
            .ok_or("Pages object missing")?;

        let pages_dict = pages_obj
            .as_dict_mut()
            .map_err(|e| format!("Pages dict error: {}", e))?;

        if let Ok(kids) = pages_dict.get_mut(b"Kids") {
            if let Ok(arr) = kids.as_array_mut() {
                for page_id in &doc_pages {
                    arr.push(Object::Reference(*page_id));
                }
            }
        }

        let total_count = (merged_count + doc_pages.len()) as i64;
        pages_dict.set("Count", Object::Integer(total_count));
    }

    save_doc(&mut merged, &output_path)
}

#[tauri::command]
pub fn rotate_pdf(req: RotatePdfRequest) -> AppResult<()> {
    let mut doc = load_doc(&req.input_path)?;
    let page_ids: Vec<ObjId> = doc.get_pages().values().copied().collect();

    for page_id in page_ids {
        if let Some(page_obj) = doc.objects.get_mut(&page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                let cur = dict
                    .get(b"Rotate")
                    .ok()
                    .and_then(|o| o.as_i64().ok())
                    .unwrap_or(0);

                dict.set(
                    "Rotate",
                    Object::Integer((cur + req.angle as i64) % 360),
                );
            }
        }
    }

    save_doc(&mut doc, &req.output_path)
}

#[tauri::command]
pub fn delete_pages(req: DeletePagesRequest) -> AppResult<()> {
    let mut doc = load_doc(&req.input_path)?;
    doc.delete_pages(&req.pages_to_delete);
    save_doc(&mut doc, &req.output_path)
}

#[tauri::command]
pub fn extract_text(path: String) -> AppResult<TextExtractResult> {
    let doc = load_doc(&path)?;
    let pages = doc.get_pages();

    let mut full_text = String::new();

    for (page_num, _) in pages.iter() {
        if let Ok(text) = doc.extract_text(&[*page_num]) {
            full_text.push_str(&format!("\n--- Page {} ---\n", page_num));
            full_text.push_str(&text);
            full_text.push('\n');
        }
    }

    Ok(TextExtractResult {
        text: full_text,
        pages: pages.len(),
    })
}

// ==================== Split PDF ====================

#[tauri::command]
pub fn split_pdf(req: SplitPdfRequest) -> AppResult<Vec<String>> {
    let doc = load_doc(&req.input_path)?;
    let total = doc.get_pages().len() as u32;
    let mut output_paths = Vec::new();

    let ranges = if req.mode == "single" {
        (1..=total).map(|p| vec![p]).collect::<Vec<_>>()
    } else {
        parse_page_ranges(&req.ranges.unwrap_or_default(), total)?
    };

    for range in &ranges {
        let mut doc_clone = doc.clone();
        let pages_to_delete: Vec<u32> = (1..=total)
            .filter(|p| !range.contains(p))
            .collect();
        if !pages_to_delete.is_empty() {
            doc_clone.delete_pages(&pages_to_delete);
        }
        let name = if range.len() == 1 {
            format!("page_{}.pdf", range[0])
        } else {
            format!("pages_{}-{}.pdf", range[0], range[range.len() - 1])
        };
        let output_path = format!("{}/{}", req.output_dir.trim_end_matches('/').trim_end_matches('\\'), name);
        save_doc(&mut doc_clone, &output_path)?;
        output_paths.push(output_path);
    }

    Ok(output_paths)
}

// ==================== Extract Pages ====================

#[tauri::command]
pub fn extract_pages_pdf(req: ExtractPagesRequest) -> AppResult<()> {
    let mut doc = load_doc(&req.input_path)?;
    let total = doc.get_pages().len() as u32;
    let pages_to_delete: Vec<u32> = (1..=total)
        .filter(|p| !req.pages_to_extract.contains(p))
        .collect();
    if !pages_to_delete.is_empty() {
        doc.delete_pages(&pages_to_delete);
    }
    save_doc(&mut doc, &req.output_path)
}

// ==================== Compress PDF ====================

#[tauri::command]
pub fn compress_pdf(input_path: String, output_path: String) -> AppResult<CompressResult> {
    let original_size = std::fs::metadata(&input_path)
        .map(|m| m.len())
        .map_err(|e| format!("Metadata error: {}", e))?;

    let mut doc = load_doc(&input_path)?;
    doc.compress();
    save_doc(&mut doc, &output_path)?;

    let compressed_size = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .map_err(|e| format!("Metadata error: {}", e))?;

    let ratio = if original_size > 0 {
        (1.0 - compressed_size as f64 / original_size as f64) * 100.0
    } else {
        0.0
    };

    Ok(CompressResult { original_size, compressed_size, ratio })
}

// ==================== Text Watermark ====================

#[tauri::command]
pub fn add_text_watermark(req: WatermarkRequest) -> AppResult<()> {
    let mut doc = load_doc(&req.input_path)?;
    let pages = doc.get_pages();

    // Parse color from hex string
    let hex = req.color.trim_start_matches('#');
    let r = u8::from_str_radix(&hex.get(0..2).unwrap_or("88"), 16).unwrap_or(136);
    let g = u8::from_str_radix(&hex.get(2..4).unwrap_or("88"), 16).unwrap_or(136);
    let b = u8::from_str_radix(&hex.get(4..6).unwrap_or("88"), 16).unwrap_or(136);

    // Create standard font
    let font_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
        (b"Type".to_vec(), Object::Name(b"Font".to_vec())),
        (b"Subtype".to_vec(), Object::Name(b"Type1".to_vec())),
        (b"BaseFont".to_vec(), Object::Name(b"Helvetica".to_vec())),
        (b"Encoding".to_vec(), Object::Name(b"WinAnsiEncoding".to_vec())),
    ])));

    for (_, page_id) in pages.iter() {
        let page = doc.get_object(*page_id).map_err(|e| format!("Page error: {}", e))?;
        let page_dict = page.as_dict().map_err(|e| format!("Page dict error: {}", e))?;
        let (pw, ph) = get_page_size(page_dict);

        let opacity = req.opacity.min(1.0).max(0.0);
        let angle_rad = req.angle.to_radians();
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();
        let cx = pw / 2.0;
        let cy = ph / 2.0;
        let escaped = escape_pdf_string(&req.text);

        // Graphics state for opacity
        let gs_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"ExtGState".to_vec())),
            (b"ca".to_vec(), Object::Real(opacity as f32)),
        ])));

        let neg_sin = -sin_a;
        let watermark_bytes = format!(
            "q /GS1 gs BT /F1 {fs:.1} Tf {cos:.4} {sin:.4} {neg_sin:.4} {cos:.4} {cx:.1} {cy:.1} Tm {r:.3} {g:.3} {b:.3} rg ({escaped}) Tj ET Q",
            fs = req.font_size, cos = cos_a, sin = sin_a, neg_sin = neg_sin, cx = cx, cy = cy,
            r = r as f64 / 255.0, g = g as f64 / 255.0, b = b as f64 / 255.0, escaped = escaped
        ).into_bytes();

        let watermark_id = doc.add_object(Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), watermark_bytes)));

        // Phase 1: get or create resources (immutable read first)
        // Handle: no Resources, Resources as reference, Resources as inline dict
        let res_ref = {
            let page = doc.get_object(*page_id).map_err(|e| format!("Page error: {}", e))?;
            let page_dict = page.as_dict().map_err(|e| format!("Page dict error: {}", e))?;
            match page_dict.get(b"Resources") {
                Err(_) => {
                    let res_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
                    (res_id, true) // needs update on page
                }
                Ok(res_obj) => {
                    if let Ok(r) = res_obj.as_reference() {
                        (r, false) // already a reference, no update needed
                    } else {
                        // Inline dictionary — promote to standalone object
                        let res_id = doc.add_object(res_obj.clone());
                        (res_id, true) // needs update on page
                    }
                }
            }
        };

        // Phase 2: add font + gs to resources
        if let Some(res_obj) = doc.objects.get_mut(&res_ref.0) {
            if let Ok(res_dict) = res_obj.as_dict_mut() {
                if res_dict.get(b"Font").is_err() {
                    res_dict.set("Font", Object::Dictionary(lopdf::Dictionary::new()));
                }
                if let Ok(font_d) = res_dict.get_mut(b"Font") {
                    if let Ok(fd) = font_d.as_dict_mut() {
                        fd.set("F1", Object::Reference(font_id));
                    }
                }
                if res_dict.get(b"ExtGState").is_err() {
                    res_dict.set("ExtGState", Object::Dictionary(lopdf::Dictionary::new()));
                }
                if let Ok(gs_d) = res_dict.get_mut(b"ExtGState") {
                    if let Ok(gd) = gs_d.as_dict_mut() {
                        gd.set("GS1", Object::Reference(gs_id));
                    }
                }
            }
        }

        // Phase 3: set resources on page if newly created
        if res_ref.1 {
            if let Some(page_obj) = doc.objects.get_mut(page_id) {
                if let Ok(dict) = page_obj.as_dict_mut() {
                    dict.set("Resources", Object::Reference(res_ref.0));
                }
            }
        }

        // Phase 4: append watermark content (two-phase to avoid double borrow)
        {
            // Phase 4a: read current contents
            let has_contents_ref: Option<ObjectId> = {
                let page_obj = doc.objects.get(page_id).unwrap();
                let dict = page_obj.as_dict().unwrap();
                match dict.get(b"Contents") {
                    Ok(c) => {
                        if let Ok(r) = c.as_reference() {
                            Some(r)
                        } else if let Ok(arr) = c.as_array() {
                            Some(doc.add_object(Object::Array(arr.clone())))
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                }
            };

            // Phase 4b: create new contents array (no borrow held)
            let new_contents_ref = match has_contents_ref {
                Some(existing_ref) => {
                    let arr = Object::Array(vec![
                        Object::Reference(existing_ref),
                        Object::Reference(watermark_id),
                    ]);
                    doc.add_object(arr)
                }
                None => watermark_id,
            };

            // Phase 4c: set contents on page
            let page_obj = doc.objects.get_mut(page_id).unwrap();
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Contents", Object::Reference(new_contents_ref));
            }
        }
    }

    save_doc(&mut doc, &req.output_path)
}

// ==================== Images to PDF ====================

#[tauri::command]
pub fn images_to_pdf(req: ImagesToPdfRequest) -> AppResult<()> {
    if req.image_paths.is_empty() {
        return Err("No images provided".into());
    }

    let mut doc = Document::with_version("1.4");
    let catalog_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
    let pages_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
        (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
        (b"Count".to_vec(), Object::Integer(req.image_paths.len() as i64)),
        (b"Kids".to_vec(), Object::Array(vec![])),
    ])));
    if let Some(cat) = doc.objects.get_mut(&catalog_id) {
        if let Ok(d) = cat.as_dict_mut() {
            d.set("Type", Object::Name(b"Catalog".to_vec()));
            d.set("Pages", Object::Reference(pages_id));
        }
    }
    doc.trailer.set(b"Root", Object::Reference(catalog_id));

    let mut kids = Vec::new();
    for image_path in &req.image_paths {
        let data = std::fs::read(image_path)
            .map_err(|e| format!("Read '{}': {}", image_path, e))?;
        let (image_id, w, h) = embed_image(&mut doc, &data, image_path)?;

        let content = format!("q {} 0 0 {} 0 0 cm /Im1 Do Q", w, h);
        let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(), content.into_bytes(),
        )));
        let resources_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"XObject".to_vec(), Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                (b"Im1".to_vec(), Object::Reference(image_id)),
            ]))),
        ])));
        let page_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Page".to_vec())),
            (b"Parent".to_vec(), Object::Reference(pages_id)),
            (b"MediaBox".to_vec(), Object::Array(vec![
                Object::Integer(0), Object::Integer(0),
                Object::Integer(w as i64), Object::Integer(h as i64),
            ])),
            (b"Contents".to_vec(), Object::Reference(content_id)),
            (b"Resources".to_vec(), Object::Reference(resources_id)),
        ])));
        kids.push(Object::Reference(page_id));
    }

    if let Some(pages_obj) = doc.objects.get_mut(&pages_id) {
        if let Ok(d) = pages_obj.as_dict_mut() {
            d.set("Kids", Object::Array(kids));
        }
    }

    save_doc(&mut doc, &req.output_path)
}

// ==================== Reorder Pages ====================

#[tauri::command]
pub fn reorder_pages(req: ReorderPagesRequest) -> AppResult<()> {
    let mut doc = load_doc(&req.input_path)?;
    let pages = doc.get_pages();
    let total = pages.len() as u32;

    if req.new_order.len() != total as usize {
        return Err(format!("Expected {} page numbers, got {}", total, req.new_order.len()));
    }

    let mut current_order: Vec<(u32, ObjectId)> = pages.iter().map(|(n, id)| (*n, *id)).collect();
    current_order.sort_by_key(|(n, _)| *n);

    let mut new_kids = Vec::new();
    for page_num in &req.new_order {
        if *page_num < 1 || *page_num > total {
            return Err(format!("Invalid page number: {}", page_num));
        }
        let (_, page_id) = current_order.iter().find(|(n, _)| *n == *page_num)
            .ok_or(format!("Page {} not found", page_num))?;
        new_kids.push(Object::Reference(*page_id));
    }

    let pages_ref = get_pages_ref(&doc)?;
    if let Some(pages_obj) = doc.objects.get_mut(&pages_ref) {
        if let Ok(dict) = pages_obj.as_dict_mut() {
            dict.set("Kids", Object::Array(new_kids));
        }
    }

    save_doc(&mut doc, &req.output_path)
}

// ==================== Insert Pages ====================

#[tauri::command]
pub fn insert_pages(req: InsertPagesRequest) -> AppResult<()> {
    let mut target = load_doc(&req.input_path)?;
    let mut source = load_doc(&req.source_path)?;

    let old_source_page_ids: Vec<ObjectId> = source.get_pages().values().copied().collect();
    let mut sorted_old_ids: Vec<ObjectId> = source.objects.keys().copied().collect();
    sorted_old_ids.sort();

    let start_id = target.max_id + 1;
    source.renumber_objects_with(start_id);

    let id_map: std::collections::BTreeMap<ObjectId, ObjectId> = sorted_old_ids
        .iter().enumerate()
        .map(|(i, old)| (*old, (start_id + i as u32, 0)))
        .collect();

    let source_page_ids: Vec<ObjectId> = old_source_page_ids
        .iter().map(|old| *id_map.get(old).unwrap_or(old)).collect();

    for (id, obj) in source.objects {
        target.objects.insert(id, obj);
    }
    if let Some(max_key) = target.objects.keys().max() {
        target.max_id = target.max_id.max(max_key.0);
    }

    let target_count = target.get_pages().len();
    let pos = req.insert_position as usize;
    if pos > target_count {
        return Err(format!("Insert position {} exceeds page count {}", pos, target_count));
    }

    let pages_ref = get_pages_ref(&target)?;
    if let Some(pages_obj) = target.objects.get_mut(&pages_ref) {
        if let Ok(dict) = pages_obj.as_dict_mut() {
            if let Ok(kids) = dict.get_mut(b"Kids") {
                if let Ok(arr) = kids.as_array_mut() {
                    for (i, page_id) in source_page_ids.iter().enumerate() {
                        arr.insert(pos + i, Object::Reference(*page_id));
                    }
                }
            }
            dict.set("Count", Object::Integer((target_count + source_page_ids.len()) as i64));
        }
    }

    save_doc(&mut target, &req.output_path)
}

// ==================== Sign PDF (visual) ====================

#[tauri::command]
pub fn sign_pdf(req: SignPdfRequest) -> AppResult<()> {
    let mut doc = load_doc(&req.input_path)?;

    let sig_data = std::fs::read(&req.signature_image_path)
        .map_err(|e| format!("Read signature: {}", e))?;
    let (image_id, _w, _h) = embed_image(&mut doc, &sig_data, &req.signature_image_path)?;

    let pages = doc.get_pages();
    let page_id = pages.get(&req.page)
        .ok_or(format!("Page {} not found", req.page))?;

    let content = format!(
        "q {} 0 0 {} {} {} cm /SigImg Do Q",
        req.width, req.height, req.x, req.y
    );
    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
        lopdf::Dictionary::new(), content.into_bytes(),
    )));

    // Get or create resources reference first
    // Handle: no Resources, Resources as reference, Resources as inline dict
    let res_ref = {
        let page = doc.get_object(*page_id).map_err(|e| format!("Page error: {}", e))?;
        let page_dict = page.as_dict().map_err(|e| format!("Page dict error: {}", e))?;
        match page_dict.get(b"Resources") {
            Err(_) => {
                let res_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                    (b"XObject".to_vec(), Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                        (b"SigImg".to_vec(), Object::Reference(image_id)),
                    ]))),
                ])));
                (res_id, true)
            }
            Ok(res_obj) => {
                if let Ok(r) = res_obj.as_reference() {
                    (r, false)
                } else {
                    // Inline dictionary — promote to standalone object
                    let res_id = doc.add_object(res_obj.clone());
                    (res_id, true)
                }
            }
        }
    };

    if !res_ref.1 {
        // Add XObject to existing resources
        if let Some(res_obj) = doc.objects.get_mut(&res_ref.0) {
            if let Ok(res_dict) = res_obj.as_dict_mut() {
                let xobject = lopdf::Dictionary::from_iter(vec![
                    (b"SigImg".to_vec(), Object::Reference(image_id)),
                ]);
                res_dict.set("XObject", Object::Dictionary(xobject));
            }
        }
    }

    // Set resources reference on page if newly created
    if res_ref.1 {
        if let Some(page_obj) = doc.objects.get_mut(page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Resources", Object::Reference(res_ref.0));
            }
        }
    }

    // Append content stream to page (two-phase to avoid double borrow)
    {
        // Phase 1: read current Contents
        let has_contents_ref: Option<ObjectId> = {
            let page_obj = doc.objects.get(page_id).unwrap();
            let dict = page_obj.as_dict().unwrap();
            match dict.get(b"Contents") {
                Ok(c) => {
                    if let Ok(r) = c.as_reference() {
                        Some(r)
                    } else if let Ok(arr) = c.as_array() {
                        Some(doc.add_object(Object::Array(arr.clone())))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        };

        // Phase 2: create new contents array (no borrow held)
        let new_contents_ref = match has_contents_ref {
            Some(existing_ref) => {
                let arr = Object::Array(vec![
                    Object::Reference(existing_ref),
                    Object::Reference(content_id),
                ]);
                doc.add_object(arr)
            }
            None => content_id,
        };

        // Phase 3: set contents on page
        let page_obj = doc.objects.get_mut(page_id).unwrap();
        if let Ok(dict) = page_obj.as_dict_mut() {
            dict.set("Contents", Object::Reference(new_contents_ref));
        }
    }

    save_doc(&mut doc, &req.output_path)
}

// ==================== OCR ====================

#[tauri::command]
pub fn check_tesseract_available() -> bool {
    std::process::Command::new("tesseract")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[tauri::command]
pub fn ocr_extract_from_images(req: OcrRequest) -> AppResult<TextExtractResult> {
    if !check_tesseract_available() {
        return Err("Tesseract OCR is not installed. Please install it from https://github.com/tesseract-ocr/tesseract".into());
    }

    let mut full_text = String::new();
    let mut page_count = 0usize;

    let mut entries: Vec<_> = std::fs::read_dir(&req.image_dir)
        .map_err(|e| format!("Read dir '{}': {}", req.image_dir, e))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("png"))
                .unwrap_or(false)
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        let path = entry.path();
        let output = std::process::Command::new("tesseract")
            .arg(&path)
            .arg("stdout")
            .arg("-l")
            .arg(&req.language)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .map_err(|e| format!("Tesseract error: {}", e))?;

        if output.status.success() {
            page_count += 1;
            let text = String::from_utf8_lossy(&output.stdout);
            full_text.push_str(&format!("\n--- Page {} ---\n", page_count));
            full_text.push_str(&text);
            full_text.push('\n');
        }
    }

    if page_count == 0 {
        return Err("No text could be extracted from the images".into());
    }

    Ok(TextExtractResult { text: full_text, pages: page_count })
}

// ==================== Temp Directory ====================

#[tauri::command]
pub fn get_temp_dir() -> AppResult<String> {
    let dir = std::env::temp_dir().join("pdf_seeker_ocr");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Create temp dir: {}", e))?;
    Ok(dir.to_string_lossy().to_string())
}

// ==================== Save Image File (bypasses fs plugin) ====================

#[tauri::command]
pub fn save_image_file(path: String, data: Vec<u8>) -> AppResult<()> {
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Create dir: {}", e))?;
    }
    std::fs::write(&path, &data)
        .map_err(|e| format!("Write '{}': {}", path, e))
}

// ==================== PDF Editing ====================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddTextRequest {
    pub input_path: String,
    pub output_path: String,
    pub text: String,
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub font_size: f64,
    pub color: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRectangleRequest {
    pub input_path: String,
    pub output_path: String,
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub border_color: String,
    pub fill_color: Option<String>,
    pub border_width: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddHighlightRequest {
    pub input_path: String,
    pub output_path: String,
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub color: String,
    pub opacity: f64,
}

#[tauri::command]
pub fn add_text_to_page(req: AddTextRequest) -> AppResult<()> {
    let mut doc = load_doc(&req.input_path)?;
    let pages = doc.get_pages();
    let page_id = pages.get(&req.page)
        .ok_or(format!("Page {} not found", req.page))?;

    let hex = req.color.trim_start_matches('#');
    let r = u8::from_str_radix(&hex.get(0..2).unwrap_or("00"), 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex.get(2..4).unwrap_or("00"), 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex.get(4..6).unwrap_or("00"), 16).unwrap_or(0);

    let font_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
        (b"Type".to_vec(), Object::Name(b"Font".to_vec())),
        (b"Subtype".to_vec(), Object::Name(b"Type1".to_vec())),
        (b"BaseFont".to_vec(), Object::Name(b"Helvetica".to_vec())),
        (b"Encoding".to_vec(), Object::Name(b"WinAnsiEncoding".to_vec())),
    ])));

    let escaped = escape_pdf_string(&req.text);
    let content = format!(
        "BT /F1 {fs:.1} Tf {r:.3} {g:.3} {b:.3} rg {x:.1} {y:.1} Td ({escaped}) Tj ET",
        fs = req.font_size,
        r = r as f64 / 255.0, g = g as f64 / 255.0, b = b as f64 / 255.0,
        x = req.x, y = req.y, escaped = escaped
    ).into_bytes();

    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), content)));

    // Handle resources
    let res_ref = {
        let page = doc.get_object(*page_id).map_err(|e| format!("Page error: {}", e))?;
        let page_dict = page.as_dict().map_err(|e| format!("Page dict error: {}", e))?;
        match page_dict.get(b"Resources") {
            Err(_) => {
                let res_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
                (res_id, true)
            }
            Ok(res_obj) => {
                if let Ok(r) = res_obj.as_reference() {
                    (r, false)
                } else {
                    let res_id = doc.add_object(res_obj.clone());
                    (res_id, true)
                }
            }
        }
    };

    // Add font to resources
    if let Some(res_obj) = doc.objects.get_mut(&res_ref.0) {
        if let Ok(res_dict) = res_obj.as_dict_mut() {
            if res_dict.get(b"Font").is_err() {
                res_dict.set("Font", Object::Dictionary(lopdf::Dictionary::new()));
            }
            if let Ok(font_d) = res_dict.get_mut(b"Font") {
                if let Ok(fd) = font_d.as_dict_mut() {
                    fd.set("F1", Object::Reference(font_id));
                }
            }
        }
    }

    if res_ref.1 {
        if let Some(page_obj) = doc.objects.get_mut(page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Resources", Object::Reference(res_ref.0));
            }
        }
    }

    // Append content
    let has_contents_ref: Option<ObjectId> = {
        let page_obj = doc.objects.get(page_id).unwrap();
        let dict = page_obj.as_dict().unwrap();
        match dict.get(b"Contents") {
            Ok(c) => {
                if let Ok(r) = c.as_reference() {
                    Some(r)
                } else if let Ok(arr) = c.as_array() {
                    Some(doc.add_object(Object::Array(arr.clone())))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    };
    let new_contents_ref = match has_contents_ref {
        Some(existing_ref) => {
            let arr = Object::Array(vec![
                Object::Reference(existing_ref),
                Object::Reference(content_id),
            ]);
            doc.add_object(arr)
        }
        None => content_id,
    };
    let page_obj = doc.objects.get_mut(page_id).unwrap();
    if let Ok(dict) = page_obj.as_dict_mut() {
        dict.set("Contents", Object::Reference(new_contents_ref));
    }

    save_doc(&mut doc, &req.output_path)
}

#[tauri::command]
pub fn add_rectangle(req: AddRectangleRequest) -> AppResult<()> {
    let mut doc = load_doc(&req.input_path)?;
    let pages = doc.get_pages();
    let page_id = pages.get(&req.page)
        .ok_or(format!("Page {} not found", req.page))?;

    let hex = req.border_color.trim_start_matches('#');
    let br = u8::from_str_radix(&hex.get(0..2).unwrap_or("00"), 16).unwrap_or(0);
    let bg = u8::from_str_radix(&hex.get(2..4).unwrap_or("00"), 16).unwrap_or(0);
    let bb = u8::from_str_radix(&hex.get(4..6).unwrap_or("00"), 16).unwrap_or(0);

    let mut content = format!(
        "{bw:.1} w {br:.3} {bg:.3} {bb:.3} RG ",
        bw = req.border_width,
        br = br as f64 / 255.0, bg = bg as f64 / 255.0, bb = bb as f64 / 255.0
    );

    if let Some(ref fill) = req.fill_color {
        let fh = fill.trim_start_matches('#');
        let fr = u8::from_str_radix(&fh.get(0..2).unwrap_or("00"), 16).unwrap_or(0);
        let fg = u8::from_str_radix(&fh.get(2..4).unwrap_or("00"), 16).unwrap_or(0);
        let fb = u8::from_str_radix(&fh.get(4..6).unwrap_or("00"), 16).unwrap_or(0);
        content.push_str(&format!(
            "{fr:.3} {fg:.3} {fb:.3} rg ",
            fr = fr as f64 / 255.0, fg = fg as f64 / 255.0, fb = fb as f64 / 255.0
        ));
        content.push_str(&format!(
            "{} {} {} {} re B Q",
            req.x, req.y, req.width, req.height
        ));
    } else {
        content.push_str(&format!(
            "{} {} {} {} re S",
            req.x, req.y, req.width, req.height
        ));
    }

    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
        lopdf::Dictionary::new(), content.into_bytes(),
    )));

    // Append content (no resources needed for basic shapes)
    let has_contents_ref: Option<ObjectId> = {
        let page_obj = doc.objects.get(page_id).unwrap();
        let dict = page_obj.as_dict().unwrap();
        match dict.get(b"Contents") {
            Ok(c) => {
                if let Ok(r) = c.as_reference() { Some(r) }
                else if let Ok(arr) = c.as_array() { Some(doc.add_object(Object::Array(arr.clone()))) }
                else { None }
            }
            Err(_) => None,
        }
    };
    let new_contents_ref = match has_contents_ref {
        Some(existing_ref) => {
            let arr = Object::Array(vec![
                Object::Reference(existing_ref),
                Object::Reference(content_id),
            ]);
            doc.add_object(arr)
        }
        None => content_id,
    };
    let page_obj = doc.objects.get_mut(page_id).unwrap();
    if let Ok(dict) = page_obj.as_dict_mut() {
        dict.set("Contents", Object::Reference(new_contents_ref));
    }

    save_doc(&mut doc, &req.output_path)
}

#[tauri::command]
pub fn add_highlight(req: AddHighlightRequest) -> AppResult<()> {
    let mut doc = load_doc(&req.input_path)?;
    let pages = doc.get_pages();
    let page_id = pages.get(&req.page)
        .ok_or(format!("Page {} not found", req.page))?;

    let hex = req.color.trim_start_matches('#');
    let r = u8::from_str_radix(&hex.get(0..2).unwrap_or("ff"), 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex.get(2..4).unwrap_or("ff"), 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex.get(4..6).unwrap_or("00"), 16).unwrap_or(0);

    // Graphics state for transparency
    let gs_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
        (b"Type".to_vec(), Object::Name(b"ExtGState".to_vec())),
        (b"ca".to_vec(), Object::Real(req.opacity.min(1.0).max(0.0) as f32)),
    ])));

    let rx = req.x;
    let ry = req.y;
    let rw = req.width;
    let rh = req.height;
    let content = format!(
        "q /GS1 gs {r:.3} {g:.3} {b:.3} rg {rx} {ry} {rw} {rh} re f Q",
        r = r as f64 / 255.0, g = g as f64 / 255.0, b = b as f64 / 255.0,
        rx = rx, ry = ry, rw = rw, rh = rh
    ).into_bytes();

    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), content)));

    // Handle resources (for ExtGState)
    let res_ref = {
        let page = doc.get_object(*page_id).map_err(|e| format!("Page error: {}", e))?;
        let page_dict = page.as_dict().map_err(|e| format!("Page dict error: {}", e))?;
        match page_dict.get(b"Resources") {
            Err(_) => {
                let res_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
                (res_id, true)
            }
            Ok(res_obj) => {
                if let Ok(r) = res_obj.as_reference() { (r, false) }
                else {
                    let res_id = doc.add_object(res_obj.clone());
                    (res_id, true)
                }
            }
        }
    };

    if let Some(res_obj) = doc.objects.get_mut(&res_ref.0) {
        if let Ok(res_dict) = res_obj.as_dict_mut() {
            if res_dict.get(b"ExtGState").is_err() {
                res_dict.set("ExtGState", Object::Dictionary(lopdf::Dictionary::new()));
            }
            if let Ok(gs_d) = res_dict.get_mut(b"ExtGState") {
                if let Ok(gd) = gs_d.as_dict_mut() {
                    gd.set("GS1", Object::Reference(gs_id));
                }
            }
        }
    }

    if res_ref.1 {
        if let Some(page_obj) = doc.objects.get_mut(page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Resources", Object::Reference(res_ref.0));
            }
        }
    }

    // Append content
    let has_contents_ref: Option<ObjectId> = {
        let page_obj = doc.objects.get(page_id).unwrap();
        let dict = page_obj.as_dict().unwrap();
        match dict.get(b"Contents") {
            Ok(c) => {
                if let Ok(r) = c.as_reference() { Some(r) }
                else if let Ok(arr) = c.as_array() { Some(doc.add_object(Object::Array(arr.clone()))) }
                else { None }
            }
            Err(_) => None,
        }
    };
    let new_contents_ref = match has_contents_ref {
        Some(existing_ref) => {
            let arr = Object::Array(vec![
                Object::Reference(existing_ref),
                Object::Reference(content_id),
            ]);
            doc.add_object(arr)
        }
        None => content_id,
    };
    let page_obj = doc.objects.get_mut(page_id).unwrap();
    if let Ok(dict) = page_obj.as_dict_mut() {
        dict.set("Contents", Object::Reference(new_contents_ref));
    }

    save_doc(&mut doc, &req.output_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a minimal valid multi-page PDF
    fn create_test_pdf(dir: &std::path::Path, name: &str, num_pages: u32) -> String {
        let path = dir.join(name);
        let mut doc = Document::with_version("1.4");

        // 1. Create catalog
        let catalog_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));

        // 2. Create pages node (empty kids for now)
        let pages_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
            (b"Count".to_vec(), Object::Integer(num_pages as i64)),
            (b"Kids".to_vec(), Object::Array(vec![])),
        ])));

        // 3. Link catalog → pages
        if let Some(cat) = doc.objects.get_mut(&catalog_id) {
            if let Ok(d) = cat.as_dict_mut() {
                d.set("Type", Object::Name(b"Catalog".to_vec()));
                d.set("Pages", Object::Reference(pages_id));
            }
        }

        // 4. Create individual page objects
        let mut kids = Vec::new();
        for _ in 0..num_pages {
            let page_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                (b"Type".to_vec(), Object::Name(b"Page".to_vec())),
                    (b"Parent".to_vec(), Object::Reference(pages_id)),
                    (b"MediaBox".to_vec(), Object::Array(vec![
                        Object::Integer(0), Object::Integer(0),
                        Object::Integer(612), Object::Integer(792),
                    ])),
                ])));
            kids.push(Object::Reference(page_id));
        }

        // 5. Update pages node with actual kids
        if let Some(pages_obj) = doc.objects.get_mut(&pages_id) {
            if let Ok(d) = pages_obj.as_dict_mut() {
                d.set("Kids", Object::Array(kids));
            }
        }

        // 6. Set trailer root
        doc.trailer.set(b"Root", Object::Reference(catalog_id));

        doc.save(&path).unwrap();
        path.to_string_lossy().to_string()
    }

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
        // Verify rotation was set
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
        // Empty test PDF should still return a result
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

        // Deleting all pages — lopdf may error or produce empty doc
        let result = delete_pages(DeletePagesRequest {
            input_path: src,
            output_path: out_str.clone(),
            pages_to_delete: vec![1, 2, 3],
        });
        // Either it fails (acceptable) or produces 0 pages
        match result {
            Ok(()) => {
                let doc = Document::load(&out_str).unwrap();
                assert_eq!(doc.get_pages().len(), 0);
            }
            Err(_) => {} // Also acceptable
        }
    }
}
