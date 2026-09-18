use image as img_crate;
use lopdf::{Document, Object, ObjectId};
use serde::Deserialize;

use crate::commands::validation::{validate_output_path, validate_path};
use crate::error::{AppError, AppResult};
use crate::pdf::io::{self, ensure_distinct_output, ValidationPolicy};

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhiteoutRequest {
    pub input_path: String,
    pub output_path: String,
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddPageNumbersRequest {
    pub input_path: String,
    pub output_path: String,
    pub format: String,
    pub position: String,
    pub start_page: Option<u32>,
    pub start_number: Option<u32>,
    pub font_size: Option<f64>,
    pub margin: Option<f64>,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditOp {
    pub op_type: String,
    pub params: serde_json::Value,
}

fn load_for_write(path: &str) -> AppResult<Document> {
    let doc = io::load_doc(path)?;
    io::reject_encrypted(&doc, path)?;
    Ok(doc)
}

pub fn escape_pdf_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

pub fn embed_image(doc: &mut Document, data: &[u8], path: &str) -> AppResult<(ObjectId, u32, u32)> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "jpg" || ext == "jpeg" {
        let img = img_crate::load_from_memory(data)
            .map_err(|e| AppError::Pdf(format!("Image decode: {}", e)))?;
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
            .map_err(|e| AppError::Pdf(format!("Image decode: {}", e)))?;
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

pub fn get_page_size(page_dict: &lopdf::Dictionary) -> (f64, f64) {
    page_dict
        .get(b"MediaBox")
        .ok()
        .and_then(|mb| mb.as_array().ok())
        .map(|arr| {
            let w = arr.get(2).and_then(|o| o.as_i64().ok()).unwrap_or(612) as f64;
            let h = arr.get(3).and_then(|o| o.as_i64().ok()).unwrap_or(792) as f64;
            (w, h)
        })
        .unwrap_or((612.0, 792.0))
}

pub fn get_page_id_by_num(doc: &Document, page_num: u32) -> AppResult<ObjectId> {
    let pages = doc.get_pages();
    pages
        .get(&page_num)
        .copied()
        .ok_or_else(|| AppError::Pdf(format!("Page {} not found", page_num)))
}

#[tauri::command]
pub fn add_text_watermark(req: WatermarkRequest) -> AppResult<()> {
    validate_path(&req.input_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(&req.output_path, &[req.input_path.as_str()])?;

    let mut doc = load_for_write(&req.input_path)?;
    let pages = doc.get_pages();
    let expected_pages = pages.len();

    let hex = req.color.trim_start_matches('#');
    let r = u8::from_str_radix(&hex.get(0..2).unwrap_or("88"), 16).unwrap_or(136);
    let g = u8::from_str_radix(&hex.get(2..4).unwrap_or("88"), 16).unwrap_or(136);
    let b = u8::from_str_radix(&hex.get(4..6).unwrap_or("88"), 16).unwrap_or(136);

    let font_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
        (b"Type".to_vec(), Object::Name(b"Font".to_vec())),
        (b"Subtype".to_vec(), Object::Name(b"Type1".to_vec())),
        (b"BaseFont".to_vec(), Object::Name(b"Helvetica".to_vec())),
        (
            b"Encoding".to_vec(),
            Object::Name(b"WinAnsiEncoding".to_vec()),
        ),
    ])));

    for (_, page_id) in pages.iter() {
        let page = doc
            .get_object(*page_id)
            .map_err(|e| AppError::Pdf(format!("Page error: {}", e)))?;
        let page_dict = page
            .as_dict()
            .map_err(|e| AppError::Pdf(format!("Page dict error: {}", e)))?;
        let (pw, ph) = get_page_size(page_dict);

        let opacity = req.opacity.min(1.0).max(0.0);
        let angle_rad = req.angle.to_radians();
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();
        let cx = pw / 2.0;
        let cy = ph / 2.0;
        let escaped = escape_pdf_string(&req.text);

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

        let watermark_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            watermark_bytes,
        )));

        let res_ref = {
            let page = doc
                .get_object(*page_id)
                .map_err(|e| AppError::Pdf(format!("Page error: {}", e)))?;
            let page_dict = page
                .as_dict()
                .map_err(|e| AppError::Pdf(format!("Page dict error: {}", e)))?;
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

        if res_ref.1 {
            if let Some(page_obj) = doc.objects.get_mut(page_id) {
                if let Ok(dict) = page_obj.as_dict_mut() {
                    dict.set("Resources", Object::Reference(res_ref.0));
                }
            }
        }

        let has_contents_ref: Option<ObjectId> = {
            let page_obj = doc
                .objects
                .get(page_id)
                .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
            let dict = page_obj.as_dict().map_err(|e| {
                AppError::Pdf(format!("Page {} is not a dictionary: {}", page_id.0, e))
            })?;
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
                    Object::Reference(watermark_id),
                ]);
                doc.add_object(arr)
            }
            None => watermark_id,
        };

        let page_obj = doc
            .objects
            .get_mut(page_id)
            .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
        if let Ok(dict) = page_obj.as_dict_mut() {
            dict.set("Contents", Object::Reference(new_contents_ref));
        }
        normalize_page_contents(&mut doc, *page_id)?;
    }

    let policy = ValidationPolicy {
        expected_pages: Some(expected_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;
    Ok(())
}

#[tauri::command]
pub fn sign_pdf(req: SignPdfRequest) -> AppResult<()> {
    validate_path(&req.input_path)?;
    validate_path(&req.signature_image_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(
        &req.output_path,
        &[req.input_path.as_str(), req.signature_image_path.as_str()],
    )?;

    let mut doc = load_for_write(&req.input_path)?;
    let expected_pages = doc.get_pages().len();

    let sig_data = std::fs::read(&req.signature_image_path)
        .map_err(|e| AppError::Pdf(format!("Read signature: {}", e)))?;
    let (image_id, _w, _h) = embed_image(&mut doc, &sig_data, &req.signature_image_path)?;

    let pages = doc.get_pages();
    let page_id = pages
        .get(&req.page)
        .ok_or_else(|| AppError::Pdf(format!("Page {} not found", req.page)))?;

    let content = format!(
        "q {} 0 0 {} {} {} cm /SigImg Do Q",
        req.width, req.height, req.x, req.y
    );
    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
        lopdf::Dictionary::new(),
        content.into_bytes(),
    )));

    let (res_needs_page_update, res_id) = {
        let page = doc
            .get_object(*page_id)
            .map_err(|e| AppError::Pdf(format!("Page error: {}", e)))?;
        let page_dict = page
            .as_dict()
            .map_err(|e| AppError::Pdf(format!("Page dict error: {}", e)))?;
        match page_dict.get(b"Resources") {
            Err(_) => {
                let res_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
                (true, res_id)
            }
            Ok(res_obj) => {
                if let Ok(r) = res_obj.as_reference() {
                    (false, r)
                } else {
                    let res_id = doc.add_object(res_obj.clone());
                    (true, res_id)
                }
            }
        }
    };

    if let Some(res_obj) = doc.objects.get_mut(&res_id) {
        if let Ok(res_dict) = res_obj.as_dict_mut() {
            if res_dict.get(b"XObject").is_err() {
                res_dict.set("XObject", Object::Dictionary(lopdf::Dictionary::new()));
            }
            if let Ok(xo_d) = res_dict.get_mut(b"XObject") {
                if let Ok(xd) = xo_d.as_dict_mut() {
                    xd.set("SigImg", Object::Reference(image_id));
                }
            }
        }
    }

    if res_needs_page_update {
        if let Some(page_obj) = doc.objects.get_mut(page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Resources", Object::Reference(res_id));
            }
        }
    }

    let has_contents_ref: Option<ObjectId> = {
        let page_obj = doc
            .objects
            .get(page_id)
            .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
        let dict = page_obj.as_dict().map_err(|e| {
            AppError::Pdf(format!("Page {} is not a dictionary: {}", page_id.0, e))
        })?;
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

    let page_obj = doc
        .objects
        .get_mut(page_id)
        .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
    if let Ok(dict) = page_obj.as_dict_mut() {
        dict.set("Contents", Object::Reference(new_contents_ref));
    }
    normalize_page_contents(&mut doc, *page_id)?;

    let policy = ValidationPolicy {
        expected_pages: Some(expected_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;
    Ok(())
}

pub fn apply_text_to_doc(
    doc: &mut Document,
    page_id: ObjectId,
    text: &str,
    x: f64,
    y: f64,
    font_size: f64,
    color: &str,
) -> AppResult<()> {
    let hex = color.trim_start_matches('#');
    let r = u8::from_str_radix(&hex.get(0..2).unwrap_or("00"), 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex.get(2..4).unwrap_or("00"), 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex.get(4..6).unwrap_or("00"), 16).unwrap_or(0);

    let font_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
        (b"Type".to_vec(), Object::Name(b"Font".to_vec())),
        (b"Subtype".to_vec(), Object::Name(b"Type1".to_vec())),
        (b"BaseFont".to_vec(), Object::Name(b"Helvetica".to_vec())),
        (
            b"Encoding".to_vec(),
            Object::Name(b"WinAnsiEncoding".to_vec()),
        ),
    ])));

    let escaped = escape_pdf_string(text);
    let content = format!(
        "BT /F1 {fs:.1} Tf {r:.3} {g:.3} {b:.3} rg {x:.1} {y:.1} Td ({escaped}) Tj ET",
        fs = font_size,
        r = r as f64 / 255.0,
        g = g as f64 / 255.0,
        b = b as f64 / 255.0,
        x = x,
        y = y,
        escaped = escaped
    )
    .into_bytes();

    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
        lopdf::Dictionary::new(),
        content,
    )));

    let res_ref = {
        let page = doc
            .get_object(page_id)
            .map_err(|e| AppError::Pdf(format!("Page error: {}", e)))?;
        let page_dict = page
            .as_dict()
            .map_err(|e| AppError::Pdf(format!("Page dict error: {}", e)))?;
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
        if let Some(page_obj) = doc.objects.get_mut(&page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Resources", Object::Reference(res_ref.0));
            }
        }
    }

    let has_contents_ref: Option<ObjectId> = {
        let page_obj = doc
            .objects
            .get(&page_id)
            .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
        let dict = page_obj
            .as_dict()
            .map_err(|e| AppError::Pdf(format!("Page {} is not a dictionary: {}", page_id.0, e)))?;
        match dict.get(b"Contents") {
            Ok(c) => {
                if let Ok(r) = c.as_reference() {
                    Some(r)
                } else if let Ok(arr) = c.as_array() {
                    Some(doc.add_object(Object::Array(arr.to_vec())))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    };
    let new_contents_ref: ObjectId = match has_contents_ref {
        Some(existing_ref) => {
            let arr = Object::Array(vec![
                Object::Reference(existing_ref),
                Object::Reference(content_id),
            ]);
            doc.add_object(arr)
        }
        None => content_id,
    };
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
    if let Ok(dict) = page_obj.as_dict_mut() {
        dict.set("Contents", Object::Reference(new_contents_ref));
    }

    Ok(())
}

pub fn apply_rectangle_to_doc(
    doc: &mut Document,
    page_id: ObjectId,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    border_color: &str,
    fill_color: Option<&str>,
    border_width: f64,
) -> AppResult<()> {
    let hex = border_color.trim_start_matches('#');
    let br = u8::from_str_radix(&hex.get(0..2).unwrap_or("00"), 16).unwrap_or(0);
    let bg = u8::from_str_radix(&hex.get(2..4).unwrap_or("00"), 16).unwrap_or(0);
    let bb = u8::from_str_radix(&hex.get(4..6).unwrap_or("00"), 16).unwrap_or(0);

    let mut content = format!("q {:.1} w", border_width);

    if let Some(fill) = fill_color {
        let fh = fill.trim_start_matches('#');
        let fr = u8::from_str_radix(&fh.get(0..2).unwrap_or("00"), 16).unwrap_or(0);
        let fg = u8::from_str_radix(&fh.get(2..4).unwrap_or("00"), 16).unwrap_or(0);
        let fb = u8::from_str_radix(&fh.get(4..6).unwrap_or("00"), 16).unwrap_or(0);
        content.push_str(&format!(
            " {:.3} {:.3} {:.3} RG {:.3} {:.3} {:.3} rg {} {} {} {} re B Q",
            br as f64 / 255.0,
            bg as f64 / 255.0,
            bb as f64 / 255.0,
            fr as f64 / 255.0,
            fg as f64 / 255.0,
            fb as f64 / 255.0,
            x,
            y,
            width,
            height
        ));
    } else {
        content.push_str(&format!(
            " {:.3} {:.3} {:.3} RG {} {} {} {} re S Q",
            br as f64 / 255.0,
            bg as f64 / 255.0,
            bb as f64 / 255.0,
            x,
            y,
            width,
            height
        ));
    }

    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
        lopdf::Dictionary::new(),
        content.into_bytes(),
    )));

    let has_contents_ref: Option<ObjectId> = {
        let page_obj = doc
            .objects
            .get(&page_id)
            .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
        let dict = page_obj
            .as_dict()
            .map_err(|e| AppError::Pdf(format!("Page {} is not a dictionary: {}", page_id.0, e)))?;
        match dict.get(b"Contents") {
            Ok(c) => {
                if let Ok(r) = c.as_reference() {
                    Some(r)
                } else if let Ok(arr) = c.as_array() {
                    Some(doc.add_object(Object::Array(arr.to_vec())))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    };
    let new_contents_ref: ObjectId = match has_contents_ref {
        Some(existing_ref) => {
            let arr = Object::Array(vec![
                Object::Reference(existing_ref),
                Object::Reference(content_id),
            ]);
            doc.add_object(arr)
        }
        None => content_id,
    };
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
    if let Ok(dict) = page_obj.as_dict_mut() {
        dict.set("Contents", Object::Reference(new_contents_ref));
    }

    Ok(())
}

pub fn apply_highlight_to_doc(
    doc: &mut Document,
    page_id: ObjectId,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: &str,
    opacity: f64,
) -> AppResult<()> {
    let hex = color.trim_start_matches('#');
    let r = u8::from_str_radix(&hex.get(0..2).unwrap_or("ff"), 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex.get(2..4).unwrap_or("ff"), 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex.get(4..6).unwrap_or("00"), 16).unwrap_or(0);

    let gs_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
        (b"Type".to_vec(), Object::Name(b"ExtGState".to_vec())),
        (
            b"ca".to_vec(),
            Object::Real(opacity.min(1.0).max(0.0) as f32),
        ),
    ])));

    let content = format!(
        "q /GS1 gs {:.3} {:.3} {:.3} rg {} {} {} {} re f Q",
        r as f64 / 255.0,
        g as f64 / 255.0,
        b as f64 / 255.0,
        x,
        y,
        width,
        height
    )
    .into_bytes();

    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
        lopdf::Dictionary::new(),
        content,
    )));

    let res_ref = {
        let page = doc
            .get_object(page_id)
            .map_err(|e| AppError::Pdf(format!("Page error: {}", e)))?;
        let page_dict = page
            .as_dict()
            .map_err(|e| AppError::Pdf(format!("Page dict error: {}", e)))?;
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
        if let Some(page_obj) = doc.objects.get_mut(&page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Resources", Object::Reference(res_ref.0));
            }
        }
    }

    let has_contents_ref: Option<ObjectId> = {
        let page_obj = doc
            .objects
            .get(&page_id)
            .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
        let dict = page_obj
            .as_dict()
            .map_err(|e| AppError::Pdf(format!("Page {} is not a dictionary: {}", page_id.0, e)))?;
        match dict.get(b"Contents") {
            Ok(c) => {
                if let Ok(r) = c.as_reference() {
                    Some(r)
                } else if let Ok(arr) = c.as_array() {
                    Some(doc.add_object(Object::Array(arr.to_vec())))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    };
    let new_contents_ref: ObjectId = match has_contents_ref {
        Some(existing_ref) => {
            let arr = Object::Array(vec![
                Object::Reference(existing_ref),
                Object::Reference(content_id),
            ]);
            doc.add_object(arr)
        }
        None => content_id,
    };
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
    if let Ok(dict) = page_obj.as_dict_mut() {
        dict.set("Contents", Object::Reference(new_contents_ref));
    }

    Ok(())
}

pub fn apply_whiteout_to_doc(
    doc: &mut Document,
    page_id: ObjectId,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> AppResult<()> {
    let content = format!("q 1 1 1 rg {} {} {} {} re f Q", x, y, width, height).into_bytes();

    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
        lopdf::Dictionary::new(),
        content,
    )));

    let has_contents_ref: Option<ObjectId> = {
        let page_obj = doc
            .objects
            .get(&page_id)
            .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
        let dict = page_obj
            .as_dict()
            .map_err(|e| AppError::Pdf(format!("Page {} is not a dictionary: {}", page_id.0, e)))?;
        match dict.get(b"Contents") {
            Ok(c) => {
                if let Ok(r) = c.as_reference() {
                    Some(r)
                } else if let Ok(arr) = c.as_array() {
                    Some(doc.add_object(Object::Array(arr.to_vec())))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    };
    let new_contents_ref: ObjectId = match has_contents_ref {
        Some(existing_ref) => {
            let arr = Object::Array(vec![
                Object::Reference(existing_ref),
                Object::Reference(content_id),
            ]);
            doc.add_object(arr)
        }
        None => content_id,
    };
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
    if let Ok(dict) = page_obj.as_dict_mut() {
        dict.set("Contents", Object::Reference(new_contents_ref));
    }

    Ok(())
}

fn flatten_contents_value(
    doc: &Document,
    value: &Object,
    flattened: &mut Vec<Object>,
) -> AppResult<()> {
    match value {
        Object::Array(items) => {
            for item in items {
                flatten_contents_value(doc, item, flattened)?;
            }
        }
        Object::Reference(id) => {
            let resolved = doc
                .get_object(*id)
                .map_err(|e| AppError::Pdf(format!("Contents object error: {}", e)))?;
            if let Ok(items) = resolved.as_array() {
                for item in items {
                    flatten_contents_value(doc, item, flattened)?;
                }
            } else {
                flattened.push(Object::Reference(*id));
            }
        }
        _ => flattened.push(value.clone()),
    }
    Ok(())
}

fn normalize_page_contents(doc: &mut Document, page_id: ObjectId) -> AppResult<()> {
    let contents = {
        let page_obj = doc
            .objects
            .get(&page_id)
            .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
        let page = page_obj
            .as_dict()
            .map_err(|e| AppError::Pdf(format!("Page {} is not a dictionary: {}", page_id.0, e)))?;
        page.get(b"Contents").ok().cloned()
    };

    let Some(contents) = contents else {
        return Ok(());
    };

    let mut flattened = Vec::new();
    flatten_contents_value(doc, &contents, &mut flattened)?;
    let normalized = match flattened.len() {
        0 => return Ok(()),
        1 => flattened.pop().expect("non-empty contents"),
        _ => Object::Array(flattened),
    };

    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or_else(|| AppError::Pdf(format!("Page object {} not found", page_id.0)))?;
    let page = page_obj
        .as_dict_mut()
        .map_err(|e| AppError::Pdf(format!("Page {} is not a dictionary: {}", page_id.0, e)))?;
    page.set("Contents", normalized);
    Ok(())
}

#[tauri::command]
pub fn add_text_to_page(req: AddTextRequest) -> AppResult<()> {
    validate_path(&req.input_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(&req.output_path, &[req.input_path.as_str()])?;

    let mut doc = load_for_write(&req.input_path)?;
    let expected_pages = doc.get_pages().len();
    let page_id = get_page_id_by_num(&doc, req.page)?;

    apply_text_to_doc(
        &mut doc,
        page_id,
        &req.text,
        req.x,
        req.y,
        req.font_size,
        &req.color,
    )?;
    normalize_page_contents(&mut doc, page_id)?;

    let policy = ValidationPolicy {
        expected_pages: Some(expected_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;
    Ok(())
}

#[tauri::command]
pub fn add_rectangle(req: AddRectangleRequest) -> AppResult<()> {
    validate_path(&req.input_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(&req.output_path, &[req.input_path.as_str()])?;

    let mut doc = load_for_write(&req.input_path)?;
    let expected_pages = doc.get_pages().len();
    let page_id = get_page_id_by_num(&doc, req.page)?;

    apply_rectangle_to_doc(
        &mut doc,
        page_id,
        req.x,
        req.y,
        req.width,
        req.height,
        &req.border_color,
        req.fill_color.as_deref(),
        req.border_width,
    )?;
    normalize_page_contents(&mut doc, page_id)?;

    let policy = ValidationPolicy {
        expected_pages: Some(expected_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;
    Ok(())
}

#[tauri::command]
pub fn add_highlight(req: AddHighlightRequest) -> AppResult<()> {
    validate_path(&req.input_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(&req.output_path, &[req.input_path.as_str()])?;

    let mut doc = load_for_write(&req.input_path)?;
    let expected_pages = doc.get_pages().len();
    let page_id = get_page_id_by_num(&doc, req.page)?;

    apply_highlight_to_doc(
        &mut doc,
        page_id,
        req.x,
        req.y,
        req.width,
        req.height,
        &req.color,
        req.opacity,
    )?;
    normalize_page_contents(&mut doc, page_id)?;

    let policy = ValidationPolicy {
        expected_pages: Some(expected_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;
    Ok(())
}

#[tauri::command]
pub fn add_whiteout(req: WhiteoutRequest) -> AppResult<()> {
    validate_path(&req.input_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(&req.output_path, &[req.input_path.as_str()])?;

    let mut doc = load_for_write(&req.input_path)?;
    let expected_pages = doc.get_pages().len();
    let page_id = get_page_id_by_num(&doc, req.page)?;

    apply_whiteout_to_doc(
        &mut doc,
        page_id,
        req.x,
        req.y,
        req.width,
        req.height,
    )?;
    normalize_page_contents(&mut doc, page_id)?;

    let policy = ValidationPolicy {
        expected_pages: Some(expected_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;
    Ok(())
}

#[tauri::command]
pub fn apply_edit_operations(
    input_path: String,
    output_path: String,
    operations: Vec<EditOp>,
) -> AppResult<()> {
    validate_path(&input_path)?;
    validate_output_path(&output_path)?;
    ensure_distinct_output(&output_path, &[input_path.as_str()])?;

    let mut doc = load_for_write(&input_path)?;
    let expected_pages = doc.get_pages().len();
    let mut edited_pages = Vec::new();

    for op in &operations {
        let page = op.params["page"].as_u64().unwrap_or(1) as u32;
        let page_id = get_page_id_by_num(&doc, page)?;
        if !edited_pages.contains(&page_id) {
            edited_pages.push(page_id);
        }

        match op.op_type.as_str() {
            "addText" => {
                apply_text_to_doc(
                    &mut doc,
                    page_id,
                    op.params["text"].as_str().unwrap_or(""),
                    op.params["x"].as_f64().unwrap_or(72.0),
                    op.params["y"].as_f64().unwrap_or(720.0),
                    op.params["fontSize"].as_f64().unwrap_or(12.0),
                    op.params["color"].as_str().unwrap_or("#000000"),
                )?;
            }
            "addRectangle" => {
                apply_rectangle_to_doc(
                    &mut doc,
                    page_id,
                    op.params["x"].as_f64().unwrap_or(100.0),
                    op.params["y"].as_f64().unwrap_or(100.0),
                    op.params["w"].as_f64().unwrap_or(200.0),
                    op.params["h"].as_f64().unwrap_or(50.0),
                    op.params["borderColor"].as_str().unwrap_or("#000000"),
                    if op.params["hasFill"].as_bool().unwrap_or(false) {
                        Some(op.params["fillColor"].as_str().unwrap_or("#ffffff"))
                    } else {
                        None
                    },
                    op.params["borderWidth"].as_f64().unwrap_or(1.0),
                )?;
            }
            "addHighlight" => {
                apply_highlight_to_doc(
                    &mut doc,
                    page_id,
                    op.params["x"].as_f64().unwrap_or(100.0),
                    op.params["y"].as_f64().unwrap_or(100.0),
                    op.params["w"].as_f64().unwrap_or(200.0),
                    op.params["h"].as_f64().unwrap_or(20.0),
                    op.params["color"].as_str().unwrap_or("#ffff00"),
                    op.params["opacity"].as_f64().unwrap_or(0.4),
                )?;
            }
            "addWhiteout" => {
                apply_whiteout_to_doc(
                    &mut doc,
                    page_id,
                    op.params["x"].as_f64().unwrap_or(0.0),
                    op.params["y"].as_f64().unwrap_or(0.0),
                    op.params["w"].as_f64().unwrap_or(100.0),
                    op.params["h"].as_f64().unwrap_or(20.0),
                )?;
            }
            other => return Err(AppError::Pdf(format!("Unknown edit operation: {}", other))),
        }
    }

    for page_id in edited_pages {
        normalize_page_contents(&mut doc, page_id)?;
    }

    let policy = ValidationPolicy {
        expected_pages: Some(expected_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &output_path, &policy)?;
    Ok(())
}

#[tauri::command]
pub fn add_page_numbers(req: AddPageNumbersRequest) -> AppResult<()> {
    validate_path(&req.input_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(&req.output_path, &[req.input_path.as_str()])?;

    let mut doc = load_for_write(&req.input_path)?;
    let pages = doc.get_pages();
    let total_pages = pages.len();
    if total_pages == 0 {
        return Err(AppError::Pdf("PDF has no pages".to_string()));
    }

    let start_page = req.start_page.unwrap_or(1).max(1);
    let start_num = req.start_number.unwrap_or(1);
    let font_size = req.font_size.unwrap_or(10.0).max(6.0).min(72.0);
    let margin = req.margin.unwrap_or(30.0).max(5.0);
    let color = req.color.unwrap_or_else(|| "#000000".to_string());

    let mut page_nums: Vec<u32> = pages.keys().cloned().collect();
    page_nums.sort_unstable();

    for (idx, page_num) in page_nums.iter().enumerate() {
        let current_page_idx = (idx + 1) as u32;
        if current_page_idx < start_page {
            continue;
        }

        let n = (current_page_idx - start_page) + start_num;
        let text = req
            .format
            .replace("{n}", &n.to_string())
            .replace("{total}", &total_pages.to_string());

        let page_id = pages[page_num];
        let page_obj = doc
            .get_object(page_id)
            .map_err(|e| AppError::Pdf(format!("Page error: {}", e)))?;
        let page_dict = page_obj
            .as_dict()
            .map_err(|e| AppError::Pdf(format!("Page dict error: {}", e)))?;
        let (pw, ph) = get_page_size(page_dict);

        let est_w = text.len() as f64 * font_size * 0.52;

        let (x, y) = match req.position.as_str() {
            "bottom-left" => (margin, margin),
            "bottom-right" => (pw - margin - est_w, margin),
            "top-left" => (margin, ph - margin - font_size),
            "top-center" => (((pw - est_w) / 2.0).max(margin), ph - margin - font_size),
            "top-right" => (pw - margin - est_w, ph - margin - font_size),
            _ => (((pw - est_w) / 2.0).max(margin), margin),
        };

        apply_text_to_doc(&mut doc, page_id, &text, x, y, font_size, &color)?;
        normalize_page_contents(&mut doc, page_id)?;
    }

    let policy = ValidationPolicy {
        expected_pages: Some(total_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;
    Ok(())
}

