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

fn obj_as_f64(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropPagesRequest {
    pub input_path: String,
    pub output_path: String,
    pub pages: Vec<u32>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Crop the given pages to the rectangle (x, y, width, height) in PDF points
/// (origin at the lower-left corner). The rectangle is clamped to the page
/// MediaBox; the CropBox is overwritten for the selected pages only.
#[tauri::command]
pub fn crop_pages(req: CropPagesRequest) -> AppResult<()> {
    if req.pages.is_empty() {
        return Err("No pages selected".into());
    }
    if req.width <= 0.0 || req.height <= 0.0 {
        return Err("Crop area must be non-empty".into());
    }

    let mut doc = load_doc(&req.input_path)?;
    let all_pages = doc.get_pages();

    for &page_num in &req.pages {
        let page_id = all_pages.get(&page_num)
            .ok_or(format!("Page {} not found", page_num))?;

        // MediaBox of the page (page-level value; fall back to the default
        // used by get_page_size when inherited)
        let media = {
            let page = doc.get_object(*page_id).map_err(|e| format!("Page error: {}", e))?;
            let page_dict = page.as_dict().map_err(|e| format!("Page dict error: {}", e))?;
            page_dict.get(b"MediaBox").ok()
                .and_then(|mb| mb.as_array().ok())
                .map(|arr| {
                    let g = |i: usize, d: f64| arr.get(i).and_then(obj_as_f64).unwrap_or(d);
                    (g(0, 0.0), g(1, 0.0), g(2, 612.0), g(3, 792.0))
                })
                .unwrap_or((0.0, 0.0, 612.0, 792.0))
        };

        // Clamp the requested rectangle to the MediaBox intersection
        let cx0 = req.x.max(media.0);
        let cy0 = req.y.max(media.1);
        let cx1 = (req.x + req.width).min(media.2);
        let cy1 = (req.y + req.height).min(media.3);
        if cx1 - cx0 <= 0.0 || cy1 - cy0 <= 0.0 {
            return Err(format!("Crop area for page {} is outside the MediaBox", page_num));
        }

        let page_obj = doc.objects.get_mut(page_id).unwrap();
        let dict = page_obj.as_dict_mut().map_err(|e| format!("Page dict error: {}", e))?;
        dict.set("CropBox", Object::Array(vec![
            Object::Real(cx0 as f32),
            Object::Real(cy0 as f32),
            Object::Real(cx1 as f32),
            Object::Real(cy1 as f32),
        ]));
    }

    save_doc(&mut doc, &req.output_path)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAnnotationRequest {
    pub input_path: String,
    pub output_path: String,
    pub page: u32,
    pub annot_type: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub color: String,
    pub opacity: f64,
    pub content: String,
}

/// Add a real PDF annotation (/Annots entry) to a page:
/// highlight | underline (with generated /AP appearance) or note (/Text,
/// rendered with the viewer's standard sticky-note icon).
#[tauri::command]
pub fn add_annotation(req: AddAnnotationRequest) -> AppResult<()> {
    let mut doc = load_doc(&req.input_path)?;
    let pages = doc.get_pages();
    let page_id = *pages.get(&req.page)
        .ok_or(format!("Page {} not found", req.page))?;

    let hex = req.color.trim_start_matches('#');
    let r = u8::from_str_radix(&hex.get(0..2).unwrap_or("ff"), 16).unwrap_or(255) as f64 / 255.0;
    let g = u8::from_str_radix(&hex.get(2..4).unwrap_or("ff"), 16).unwrap_or(255) as f64 / 255.0;
    let b = u8::from_str_radix(&hex.get(4..6).unwrap_or("00"), 16).unwrap_or(0) as f64 / 255.0;

    let (subtype, ap_id) = match req.annot_type.as_str() {
        "highlight" => {
            let gs_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                (b"Type".to_vec(), Object::Name(b"ExtGState".to_vec())),
                (b"ca".to_vec(), Object::Real(req.opacity.min(1.0).max(0.0) as f32)),
            ])));
            let content = format!(
                "q /GS0 gs {r:.3} {g:.3} {b:.3} rg 0 0 {w:.1} {h:.1} re f Q",
                r = r, g = g, b = b, w = req.width, h = req.height
            );
            let res = lopdf::Dictionary::from_iter(vec![(
                b"ExtGState".to_vec(),
                Object::Dictionary(lopdf::Dictionary::from_iter(vec![(
                    b"GS0".to_vec(),
                    Object::Reference(gs_id),
                )])),
            )]);
            let ap_dict = lopdf::Dictionary::from_iter(vec![
                (b"BBox".to_vec(), Object::Array(vec![
                    Object::Integer(0), Object::Integer(0),
                    Object::Real(req.width as f32), Object::Real(req.height as f32),
                ])),
                (b"Resources".to_vec(), Object::Dictionary(res)),
            ]);
            let id = doc.add_object(Object::Stream(lopdf::Stream::new(ap_dict, content.into_bytes())));
            (b"Highlight".to_vec(), Some(id))
        }
        "underline" => {
            let content = format!(
                "q {r:.3} {g:.3} {b:.3} RG 1.5 w 0 1 m {w:.1} 1 l S Q",
                r = r, g = g, b = b, w = req.width
            );
            let ap_dict = lopdf::Dictionary::from_iter(vec![
                (b"BBox".to_vec(), Object::Array(vec![
                    Object::Integer(0), Object::Integer(0),
                    Object::Real(req.width as f32), Object::Real(req.height.max(3.0) as f32),
                ])),
                (b"Resources".to_vec(), Object::Dictionary(lopdf::Dictionary::new())),
            ]);
            let id = doc.add_object(Object::Stream(lopdf::Stream::new(ap_dict, content.into_bytes())));
            (b"Underline".to_vec(), Some(id))
        }
        "note" => {
            // /Text annotation: no /AP — the viewer draws its standard icon
            (b"Text".to_vec(), None)
        }
        other => return Err(format!("Unknown annotation type: {}", other)),
    };

    let mut annot = lopdf::Dictionary::from_iter(vec![
        (b"Type".to_vec(), Object::Name(b"Annot".to_vec())),
        (b"Subtype".to_vec(), Object::Name(subtype)),
        (b"Rect".to_vec(), Object::Array(vec![
            Object::Real(req.x as f32),
            Object::Real(req.y as f32),
            Object::Real((req.x + req.width) as f32),
            Object::Real((req.y + req.height) as f32),
        ])),
        (b"C".to_vec(), Object::Array(vec![
            Object::Real(r as f32), Object::Real(g as f32), Object::Real(b as f32),
        ])),
        (b"F".to_vec(), Object::Integer(4)),
        (b"T".to_vec(), Object::String(b"PDF Seeker".to_vec(), lopdf::StringFormat::Literal)),
    ]);
    if !req.content.is_empty() {
        annot.set(b"Contents", Object::String(req.content.clone().into_bytes(), lopdf::StringFormat::Literal));
    }
    if let Some(id) = ap_id {
        annot.set(b"AP", Object::Dictionary(lopdf::Dictionary::from_iter(vec![(
            b"N".to_vec(),
            Object::Reference(id),
        )])));
    }

    let annot_id = doc.add_object(Object::Dictionary(annot));

    // Append to the page /Annots array (create or extend, inline or referenced)
    let existing_annots: Option<Object> = {
        let page = doc.get_object(page_id).map_err(|e| format!("Page error: {}", e))?;
        let page_dict = page.as_dict().map_err(|e| format!("Page dict error: {}", e))?;
        match page_dict.get(b"Annots") {
            Ok(Object::Reference(r)) => doc.get_object(*r)
                .map_err(|e| format!("Annots error: {}", e))?
                .as_array()
                .map(|a| Object::Array(a.clone()))
                .ok(),
            Ok(o) => o.as_array().map(|a| Object::Array(a.clone())).ok(),
            Err(_) => None,
        }
    };

    let mut new_annots = match existing_annots {
        Some(Object::Array(a)) => a,
        _ => Vec::new(),
    };
    new_annots.push(Object::Reference(annot_id));

    let page_obj = doc.objects.get_mut(&page_id).unwrap();
    if let Ok(dict) = page_obj.as_dict_mut() {
        dict.set("Annots", Object::Array(new_annots));
    }

    save_doc(&mut doc, &req.output_path)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormField {
    pub name: String,
    pub field_type: String,
    pub value: String,
    pub options: Vec<String>,
    pub page_index: u32,
}

fn object_to_display_string(o: &Object) -> String {
    match o {
        Object::String(bytes, _) => String::from_utf8_lossy(bytes).to_string(),
        Object::Name(n) => String::from_utf8_lossy(n).to_string(),
        Object::Integer(i) => i.to_string(),
        Object::Real(r) => r.to_string(),
        _ => String::new(),
    }
}

fn field_type_name(field_dict: &lopdf::Dictionary) -> &'static str {
    let flags = field_dict.get(b"Ff").ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0);
    match field_dict.get(b"FT").ok().and_then(|o| o.as_name().ok()) {
        Some(b"Tx") => "text",
        Some(b"Btn") => {
            if flags & 0x8000 != 0 { "radio" }     // bit 16: radio
            else if flags & 0x10000 != 0 { "button" } // bit 17: pushbutton
            else { "checkbox" }
        }
        Some(b"Ch") => "choice",
        Some(b"Sig") => "signature",
        _ => "unknown",
    }
}

fn field_options(field_dict: &lopdf::Dictionary) -> Vec<String> {
    field_dict.get(b"Opt").ok()
        .and_then(|o| o.as_array().ok())
        .map(|arr| {
            arr.iter().filter_map(|opt| {
                match opt {
                    // Choice options may be [export_value, label] pairs
                    Object::Array(pair) => pair.first().map(object_to_display_string),
                    other => Some(object_to_display_string(other)),
                }
            }).collect()
        })
        .unwrap_or_default()
}

/// Resolve the Catalog /AcroForm dictionary (following a reference if needed)
/// and return it plus a marker for how it is attached. Returns the (root_ref,
/// acroform_dict) pair.
fn get_acroform(doc: &Document) -> Option<(ObjectId, lopdf::Dictionary)> {
    let root_ref = doc.trailer.get(b"Root").ok()?.as_reference().ok()?;
    let root = doc.get_object(root_ref).ok()?.as_dict().ok()?;
    match root.get(b"AcroForm").ok()? {
        Object::Reference(r) => {
            let d = doc.get_object(*r).ok()?.as_dict().ok()?.clone();
            Some((root_ref, d))
        }
        Object::Dictionary(d) => Some((root_ref, d.clone())),
        _ => None,
    }
}

/// Walk /Fields recursively. Terminal fields carry /FT (or have only widget
/// kids); intermediate nodes have /T + /Kids of sub-fields and get a
/// "parent.child" name prefix.
fn walk_fields(
    doc: &Document,
    entries: &[Object],
    prefix: &str,
    page_lookup: &std::collections::HashMap<ObjectId, u32>,
    out: &mut Vec<FormField>,
) {
    for entry in entries {
        let dict = match entry {
            Object::Reference(r) => match doc.get_object(*r) {
                Ok(Object::Dictionary(d)) => d,
                _ => continue,
            },
            Object::Dictionary(d) => d,
            _ => continue,
        };

        let own_name = dict.get(b"T").ok()
            .map(object_to_display_string)
            .unwrap_or_default();
        let qualified = if prefix.is_empty() { own_name.clone() } else { format!("{}.{}", prefix, own_name) };

        // Sub-fields: kids that themselves have /T
        let has_field_kids = dict.get(b"Kids").ok()
            .and_then(|o| o.as_array().ok())
            .map(|kids| kids.iter().any(|k| {
                let kd = match k {
                    Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
                    Object::Dictionary(d) => Some(d),
                    _ => None,
                };
                kd.map(|d| d.get(b"T").is_ok()).unwrap_or(false)
            }))
            .unwrap_or(false);

        if has_field_kids {
            let kids = dict.get(b"Kids").ok().and_then(|o| o.as_array().ok()).cloned().unwrap_or_default();
            walk_fields(doc, &kids, &qualified, page_lookup, out);
            continue;
        }

        if dict.get(b"FT").is_err() {
            continue; // not a terminal field
        }

        let page_index = match dict.get(b"P").ok().and_then(|o| o.as_reference().ok()) {
            Some(p) => page_lookup.get(&p).copied().unwrap_or(0),
            None => 0,
        };

        out.push(FormField {
            name: qualified,
            field_type: field_type_name(dict).to_string(),
            value: dict.get(b"V").ok().map(object_to_display_string).unwrap_or_default(),
            options: field_options(dict),
            page_index,
        });
    }
}

#[tauri::command]
pub fn get_form_fields(path: String) -> AppResult<Vec<FormField>> {
    let doc = load_doc(&path)?;
    let (_, acroform) = get_acroform(&doc)
        .ok_or("This PDF has no AcroForm (no fillable form fields)")?;

    let mut page_lookup = std::collections::HashMap::new();
    for (num, id) in doc.get_pages() {
        page_lookup.insert(id, num);
    }

    let entries = acroform.get(b"Fields").ok()
        .and_then(|o| o.as_array().ok())
        .cloned()
        .ok_or("AcroForm has no /Fields array")?;

    let mut fields = Vec::new();
    walk_fields(&doc, &entries, "", &page_lookup, &mut fields);
    if fields.is_empty() {
        return Err("No form fields found".into());
    }
    Ok(fields)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormFieldValue {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillFormRequest {
    pub input_path: String,
    pub output_path: String,
    pub values: Vec<FormFieldValue>,
}

/// Locate terminal field dictionaries by fully-qualified name.
/// Returns (field object id, field type) pairs; only referenced fields can be
/// modified in place.
fn locate_terminal_fields(
    doc: &Document,
    entries: &[Object],
    prefix: &str,
    out: &mut Vec<(String, Option<ObjectId>, String)>,
) {
    for entry in entries {
        let (obj_id, dict) = match entry {
            Object::Reference(r) => match doc.get_object(*r) {
                Ok(Object::Dictionary(d)) => (Some(*r), d),
                _ => continue,
            },
            Object::Dictionary(d) => (None, d),
            _ => continue,
        };

        let own_name = dict.get(b"T").ok().map(object_to_display_string).unwrap_or_default();
        let qualified = if prefix.is_empty() { own_name.clone() } else { format!("{}.{}", prefix, own_name) };

        let has_field_kids = dict.get(b"Kids").ok()
            .and_then(|o| o.as_array().ok())
            .map(|kids| kids.iter().any(|k| {
                let kd = match k {
                    Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
                    Object::Dictionary(d) => Some(d),
                    _ => None,
                };
                kd.map(|d| d.get(b"T").is_ok()).unwrap_or(false)
            }))
            .unwrap_or(false);

        if has_field_kids {
            let kids = dict.get(b"Kids").ok().and_then(|o| o.as_array().ok()).cloned().unwrap_or_default();
            locate_terminal_fields(doc, &kids, &qualified, out);
        } else if dict.get(b"FT").is_ok() {
            out.push((qualified, obj_id, field_type_name(dict).to_string()));
        }
    }
}

#[tauri::command]
pub fn fill_form(req: FillFormRequest) -> AppResult<()> {
    let mut doc = load_doc(&req.input_path)?;
    let (root_ref, _) = get_acroform(&doc)
        .ok_or("This PDF has no AcroForm (no fillable form fields)")?;

    let entries = {
        let root = doc.get_object(root_ref).map_err(|e| format!("Catalog error: {}", e))?;
        let acroform = match root.as_dict().map_err(|e| format!("Catalog error: {}", e))?.get(b"AcroForm") {
            Ok(Object::Reference(r)) => doc.get_object(*r)
                .map_err(|e| format!("AcroForm error: {}", e))?
                .as_dict()
                .map_err(|e| format!("AcroForm error: {}", e))?
                .get(b"Fields").ok()
                .and_then(|o| o.as_array().ok())
                .cloned()
                .ok_or("AcroForm has no /Fields array")?,
            Ok(_) => return Err("Unsupported inline AcroForm".into()),
            Err(_) => return Err("AcroForm has no /Fields array".into()),
        };
        acroform
    };

    let mut located = Vec::new();
    locate_terminal_fields(&doc, &entries, "", &mut located);

    let truthy = |v: &str| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes" | "on" | "checked");

    let mut applied = 0;
    for value in &req.values {
        let (_, obj_id, ftype) = match located.iter().find(|(name, _, _)| *name == value.name) {
            Some(l) => l,
            None => return Err(format!("Form field '{}' not found", value.name)),
        };
        let obj_id = obj_id.ok_or(format!("Form field '{}' is not a reference and cannot be filled", value.name))?;

        let field_obj = doc.objects.get_mut(&obj_id).unwrap();
        let dict = field_obj.as_dict_mut().map_err(|e| format!("Field dict error: {}", e))?;
        match ftype.as_str() {
            "text" | "choice" => {
                dict.set(b"V", Object::String(value.value.clone().into_bytes(), lopdf::StringFormat::Literal));
            }
            "checkbox" => {
                let state = if truthy(&value.value) { b"Yes".to_vec() } else { b"Off".to_vec() };
                dict.set(b"V", Object::Name(state.clone()));
                dict.set(b"AS", Object::Name(state));
            }
            "radio" => {
                dict.set(b"V", Object::Name(value.value.clone().into_bytes()));
                dict.set(b"AS", Object::Name(value.value.clone().into_bytes()));
            }
            other => return Err(format!("Field '{}' of type '{}' cannot be filled", value.name, other)),
        }
        applied += 1;
    }

    if applied == 0 {
        return Err("No values to fill".into());
    }

    // Ask viewers to rebuild field appearances for the new values.
    // Two-phase: read how AcroForm is attached, then mutate without overlap.
    let acroform_ref = {
        let root_obj = doc.objects.get(&root_ref).unwrap();
        root_obj.as_dict().unwrap().get(b"AcroForm").ok()
            .and_then(|o| o.as_reference().ok())
    };
    match acroform_ref {
        Some(r) => {
            let acro_obj = doc.objects.get_mut(&r).unwrap();
            if let Ok(acro) = acro_obj.as_dict_mut() {
                acro.set(b"NeedAppearances", Object::Boolean(true));
            }
        }
        None => {
            // Inline AcroForm dictionary on the catalog
            let root_obj = doc.objects.get_mut(&root_ref).unwrap();
            if let Ok(root_dict) = root_obj.as_dict_mut() {
                if let Ok(acro) = root_dict.get_mut(b"AcroForm") {
                    if let Ok(acro_dict) = acro.as_dict_mut() {
                        acro_dict.set(b"NeedAppearances", Object::Boolean(true));
                    }
                }
            }
        }
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

    #[test]
    fn test_crop_pages() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "c.pdf", 2);
        let out = dir.path().join("cropped.pdf");
        let out_str = out.to_string_lossy().to_string();

        crop_pages(CropPagesRequest {
            input_path: src,
            output_path: out_str.clone(),
            pages: vec![1],
            x: 50.0,
            y: 100.0,
            width: 400.0,
            height: 500.0,
        })
        .unwrap();

        let doc = Document::load(&out_str).unwrap();
        let pages = doc.get_pages();

        // Page 1 has the new CropBox
        let id1 = *pages.get(&1).unwrap();
        let dict1 = doc.get_object(id1).unwrap().as_dict().unwrap();
        let cb = dict1.get(b"CropBox").unwrap().as_array().unwrap();
        let vals: Vec<f64> = cb.iter().map(|o| obj_as_f64(o).unwrap()).collect();
        assert!((vals[0] - 50.0).abs() < 0.1, "x0 = {}", vals[0]);
        assert!((vals[1] - 100.0).abs() < 0.1, "y0 = {}", vals[1]);
        assert!((vals[2] - 450.0).abs() < 0.1, "x1 = {}", vals[2]);
        assert!((vals[3] - 600.0).abs() < 0.1, "y1 = {}", vals[3]);

        // Page 2 is untouched
        let id2 = *pages.get(&2).unwrap();
        let dict2 = doc.get_object(id2).unwrap().as_dict().unwrap();
        assert!(dict2.get(b"CropBox").is_err());
    }

    #[test]
    fn test_crop_outside_mediabox_fails() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "co.pdf", 1);

        // Rect completely outside the 612x792 MediaBox
        let result = crop_pages(CropPagesRequest {
            input_path: src,
            output_path: dir.path().join("nope.pdf").to_string_lossy().to_string(),
            pages: vec![1],
            x: 700.0,
            y: 800.0,
            width: 100.0,
            height: 100.0,
        });
        assert!(result.is_err());

        // Zero-size crop also fails
        let result = crop_pages(CropPagesRequest {
            input_path: create_test_pdf(dir.path(), "cz.pdf", 1),
            output_path: dir.path().join("nope2.pdf").to_string_lossy().to_string(),
            pages: vec![1],
            x: 10.0,
            y: 10.0,
            width: 0.0,
            height: 10.0,
        });
        assert!(result.is_err());
    }

    /// Resolve the page /Annots array of the first page
    fn get_first_page_annots(doc: &Document) -> Vec<Object> {
        let page_id = *doc.get_pages().get(&1).unwrap();
        let dict = doc.get_object(page_id).unwrap().as_dict().unwrap();
        match dict.get(b"Annots").unwrap() {
            Object::Reference(r) => doc.get_object(*r).unwrap().as_array().unwrap().clone(),
            Object::Array(a) => a.clone(),
            o => panic!("Unexpected Annots: {:?}", o),
        }
    }

    #[test]
    fn test_add_highlight_annotation() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "ann.pdf", 1);
        let out = dir.path().join("ann_out.pdf");
        let out_str = out.to_string_lossy().to_string();

        add_annotation(AddAnnotationRequest {
            input_path: src,
            output_path: out_str.clone(),
            page: 1,
            annot_type: "highlight".into(),
            x: 100.0,
            y: 200.0,
            width: 300.0,
            height: 24.0,
            color: "#ffff00".into(),
            opacity: 0.4,
            content: String::new(),
        })
        .unwrap();

        let doc = Document::load(&out_str).unwrap();
        let annots = get_first_page_annots(&doc);
        assert_eq!(annots.len(), 1);
        let annot_ref = annots[0].as_reference().unwrap();
        let annot = doc.get_object(annot_ref).unwrap().as_dict().unwrap();
        assert_eq!(annot.get(b"Subtype").unwrap().as_name().unwrap(), b"Highlight");
        let rect = annot.get(b"Rect").unwrap().as_array().unwrap();
        let vals: Vec<f64> = rect.iter().map(|o| obj_as_f64(o).unwrap()).collect();
        assert!((vals[0] - 100.0).abs() < 0.1);
        assert!((vals[3] - 224.0).abs() < 0.1);
        // AP appearance stream present
        assert!(annot.get(b"AP").is_ok());
    }

    #[test]
    fn test_add_note_annotation_and_underline() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "ann2.pdf", 1);
        let out = dir.path().join("ann2_out.pdf");
        let out_str = out.to_string_lossy().to_string();

        add_annotation(AddAnnotationRequest {
            input_path: src.clone(),
            output_path: out_str.clone(),
            page: 1,
            annot_type: "note".into(),
            x: 72.0,
            y: 700.0,
            width: 24.0,
            height: 24.0,
            color: "#ffd54f".into(),
            opacity: 1.0,
            content: "Check this".into(),
        })
        .unwrap();
        add_annotation(AddAnnotationRequest {
            input_path: out_str.clone(),
            output_path: out_str.clone(),
            page: 1,
            annot_type: "underline".into(),
            x: 72.0,
            y: 660.0,
            width: 200.0,
            height: 12.0,
            color: "#ff0000".into(),
            opacity: 1.0,
            content: String::new(),
        })
        .unwrap();

        let doc = Document::load(&out_str).unwrap();
        let annots = get_first_page_annots(&doc);
        assert_eq!(annots.len(), 2);

        let note = doc.get_object(annots[0].as_reference().unwrap()).unwrap().as_dict().unwrap();
        assert_eq!(note.get(b"Subtype").unwrap().as_name().unwrap(), b"Text");
        match note.get(b"Contents").unwrap() {
            Object::String(bytes, _) => assert_eq!(bytes, b"Check this"),
            o => panic!("Unexpected Contents: {:?}", o),
        }

        let ul = doc.get_object(annots[1].as_reference().unwrap()).unwrap().as_dict().unwrap();
        assert_eq!(ul.get(b"Subtype").unwrap().as_name().unwrap(), b"Underline");
    }

    #[test]
    fn test_add_annotation_bad_type_fails() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "ann3.pdf", 1);
        let result = add_annotation(AddAnnotationRequest {
            input_path: src,
            output_path: dir.path().join("x.pdf").to_string_lossy().to_string(),
            page: 1,
            annot_type: "circle".into(),
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            color: "#000000".into(),
            opacity: 1.0,
            content: String::new(),
        });
        assert!(result.is_err());
    }

    /// Create a 1-page PDF with an AcroForm containing a text field, a
    /// checkbox and a radio group
    fn create_form_test_pdf(dir: &std::path::Path, name: &str) -> String {
        let path = dir.join(name);
        let mut doc = Document::with_version("1.4");

        let catalog_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
        let pages_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
            (b"Count".to_vec(), Object::Integer(1)),
            (b"Kids".to_vec(), Object::Array(vec![])),
        ])));
        if let Some(cat) = doc.objects.get_mut(&catalog_id) {
            if let Ok(d) = cat.as_dict_mut() {
                d.set("Type", Object::Name(b"Catalog".to_vec()));
                d.set("Pages", Object::Reference(pages_id));
            }
        }
        let page_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Page".to_vec())),
            (b"Parent".to_vec(), Object::Reference(pages_id)),
            (b"MediaBox".to_vec(), Object::Array(vec![
                Object::Integer(0), Object::Integer(0),
                Object::Integer(612), Object::Integer(792),
            ])),
        ])));
        if let Some(pages_obj) = doc.objects.get_mut(&pages_id) {
            if let Ok(d) = pages_obj.as_dict_mut() {
                d.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
            }
        }
        doc.trailer.set(b"Root", Object::Reference(catalog_id));

        let text_field_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"FT".to_vec(), Object::Name(b"Tx".to_vec())),
            (b"T".to_vec(), Object::String(b"fullname".to_vec(), lopdf::StringFormat::Literal)),
            (b"V".to_vec(), Object::String(b"old value".to_vec(), lopdf::StringFormat::Literal)),
            (b"P".to_vec(), Object::Reference(page_id)),
        ])));
        let checkbox_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"FT".to_vec(), Object::Name(b"Btn".to_vec())),
            (b"T".to_vec(), Object::String(b"subscribe".to_vec(), lopdf::StringFormat::Literal)),
            (b"V".to_vec(), Object::Name(b"Off".to_vec())),
            (b"AS".to_vec(), Object::Name(b"Off".to_vec())),
            (b"P".to_vec(), Object::Reference(page_id)),
        ])));
        let radio_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"FT".to_vec(), Object::Name(b"Btn".to_vec())),
            (b"Ff".to_vec(), Object::Integer(0x8000)), // radio flag
            (b"T".to_vec(), Object::String(b"color".to_vec(), lopdf::StringFormat::Literal)),
            (b"V".to_vec(), Object::Name(b"red".to_vec())),
            (b"Opt".to_vec(), Object::Array(vec![
                Object::String(b"red".to_vec(), lopdf::StringFormat::Literal),
                Object::String(b"green".to_vec(), lopdf::StringFormat::Literal),
            ])),
            (b"P".to_vec(), Object::Reference(page_id)),
        ])));

        let acroform_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Fields".to_vec(), Object::Array(vec![
                Object::Reference(text_field_id),
                Object::Reference(checkbox_id),
                Object::Reference(radio_id),
            ])),
        ])));
        if let Some(cat) = doc.objects.get_mut(&catalog_id) {
            if let Ok(d) = cat.as_dict_mut() {
                d.set("AcroForm", Object::Reference(acroform_id));
            }
        }

        doc.save(&path).unwrap();
        path.to_string_lossy().to_string()
    }

    #[test]
    fn test_get_form_fields() {
        let dir = TempDir::new().unwrap();
        let src = create_form_test_pdf(dir.path(), "form.pdf");

        let fields = get_form_fields(src).unwrap();
        assert_eq!(fields.len(), 3);

        let text = fields.iter().find(|f| f.name == "fullname").unwrap();
        assert_eq!(text.field_type, "text");
        assert_eq!(text.value, "old value");
        assert_eq!(text.page_index, 1);

        let check = fields.iter().find(|f| f.name == "subscribe").unwrap();
        assert_eq!(check.field_type, "checkbox");
        assert_eq!(check.value, "Off");

        let radio = fields.iter().find(|f| f.name == "color").unwrap();
        assert_eq!(radio.field_type, "radio");
        assert_eq!(radio.value, "red");
        assert_eq!(radio.options, vec!["red".to_string(), "green".to_string()]);
    }

    #[test]
    fn test_fill_form() {
        let dir = TempDir::new().unwrap();
        let src = create_form_test_pdf(dir.path(), "form_fill.pdf");
        let out = dir.path().join("form_filled.pdf");
        let out_str = out.to_string_lossy().to_string();

        fill_form(FillFormRequest {
            input_path: src,
            output_path: out_str.clone(),
            values: vec![
                FormFieldValue { name: "fullname".into(), value: "Ada Lovelace".into() },
                FormFieldValue { name: "subscribe".into(), value: "true".into() },
                FormFieldValue { name: "color".into(), value: "green".into() },
            ],
        })
        .unwrap();

        // Verify via get_form_fields roundtrip
        let fields = get_form_fields(out_str.clone()).unwrap();
        assert_eq!(fields.iter().find(|f| f.name == "fullname").unwrap().value, "Ada Lovelace");
        assert_eq!(fields.iter().find(|f| f.name == "subscribe").unwrap().value, "Yes");
        assert_eq!(fields.iter().find(|f| f.name == "color").unwrap().value, "green");

        // NeedAppearances set on the AcroForm
        let doc = Document::load(&out_str).unwrap();
        let root_ref = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let root = doc.get_object(root_ref).unwrap().as_dict().unwrap();
        let acro_ref = root.get(b"AcroForm").unwrap().as_reference().unwrap();
        let acro = doc.get_object(acro_ref).unwrap().as_dict().unwrap();
        assert!(matches!(acro.get(b"NeedAppearances"), Ok(Object::Boolean(true))));
    }

    #[test]
    fn test_fill_form_no_form_fails() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "noform.pdf", 1);
        let result = fill_form(FillFormRequest {
            input_path: src,
            output_path: dir.path().join("nope.pdf").to_string_lossy().to_string(),
            values: vec![FormFieldValue { name: "x".into(), value: "y".into() }],
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_get_form_fields_no_form_fails() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "noform2.pdf", 1);
        assert!(get_form_fields(src).is_err());
    }
}
