use lopdf::{Document, Object};
use serde::Serialize;

use crate::commands::validation::{validate_output_path, validate_path};
use crate::error::{AppError, AppResult};
use crate::pdf::io::{self, ensure_distinct_output, ValidationPolicy};

#[derive(Debug, Serialize)]
pub struct CompressResult {
    pub original_size: u64,
    pub compressed_size: u64,
    pub ratio: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfInfoResult {
    pub is_encrypted: bool,
    pub pages: u32,
    pub file_size: u64,
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<String>,
    pub mod_date: Option<String>,
    pub pdf_version: Option<String>,
    pub page_size: Option<String>,
    pub image_count: u32,
}

fn extract_info_string(d: &Document, key: &[u8]) -> Option<String> {
    let info_obj = d.trailer.get(b"Info").ok().and_then(|obj| match obj {
        Object::Reference(id) => d.objects.get(id),
        Object::Dictionary(_) => Some(obj),
        _ => None,
    })?;

    let dict = match info_obj {
        Object::Dictionary(ref dict) => dict,
        _ => return None,
    };

    let val = dict.get(key).ok()?;
    let bytes = val.as_str().ok()?;
    if bytes.is_empty() {
        return None;
    }
    // Handle UTF-16BE encoding with BOM
    if bytes.len() >= 2 && bytes[0] == 0xfe && bytes[1] == 0xff {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16(&u16s).ok()
    } else {
        Some(String::from_utf8_lossy(bytes).into_owned())
    }
}

#[tauri::command]
pub fn compress_pdf(input_path: String, output_path: String) -> AppResult<CompressResult> {
    validate_path(&input_path)?;
    validate_output_path(&output_path)?;
    ensure_distinct_output(&output_path, &[input_path.as_str()])?;

    let original_size = std::fs::metadata(&input_path)
        .map(|m| m.len())
        .map_err(AppError::from)?;

    let mut doc = io::load_doc(&input_path)?;
    io::reject_encrypted(&doc, &input_path)?;
    let expected_pages = doc.get_pages().len();

    doc.compress();

    let policy = ValidationPolicy {
        expected_pages: Some(expected_pages),
        check_page_tree: true,
    };
    let write_res = io::write_transactional(&mut doc, &output_path, &policy)?;
    let compressed_size = write_res.output_size;

    let ratio = if original_size > 0 {
        (1.0 - compressed_size as f64 / original_size as f64) * 100.0
    } else {
        0.0
    };

    Ok(CompressResult {
        original_size,
        compressed_size,
        ratio,
    })
}

#[tauri::command]
pub fn get_temp_dir() -> AppResult<String> {
    let dir = std::env::temp_dir().join("pdf_seeker_ocr");
    std::fs::create_dir_all(&dir).map_err(AppError::from)?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn save_image_file(path: String, data: Vec<u8>) -> AppResult<()> {
    let path_obj = std::path::Path::new(&path);
    if let Some(parent) = path_obj.parent() {
        validate_path(&parent.to_string_lossy())?;
        std::fs::create_dir_all(parent).map_err(AppError::from)?;
    }
    std::fs::write(&path, &data).map_err(|e| AppError::Pdf(format!("Write '{}': {}", path, e)))
}

#[tauri::command]
pub fn get_pdf_info(path: String) -> AppResult<PdfInfoResult> {
    validate_path(&path)?;
    let metadata = std::fs::metadata(&path).map_err(AppError::from)?;
    let file_size = metadata.len();

    let doc = Document::load(&path);

    match doc {
        Ok(d) => {
            let is_encrypted = d.is_encrypted();
            let pages = if is_encrypted {
                0
            } else {
                d.get_pages().len() as u32
            };

            let title = extract_info_string(&d, b"Title");
            let author = extract_info_string(&d, b"Author");
            let subject = extract_info_string(&d, b"Subject");
            let keywords = extract_info_string(&d, b"Keywords");
            let creator = extract_info_string(&d, b"Creator");
            let producer = extract_info_string(&d, b"Producer");
            let creation_date = extract_info_string(&d, b"CreationDate");
            let mod_date = extract_info_string(&d, b"ModDate");
            let pdf_version = Some(d.version.clone());

            let page_size = d.get_pages().iter().next().and_then(|(_, &page_id)| {
                let page_obj = d.objects.get(&page_id)?;
                let dict = page_obj.as_dict().ok()?;
                let box_obj = dict.get(b"MediaBox").ok()?;
                let arr = box_obj.as_array().ok()?;
                if arr.len() >= 4 {
                    let x0 = arr[0]
                        .as_float()
                        .or_else(|_| arr[0].as_i64().map(|v| v as f32))
                        .ok()?;
                    let y0 = arr[1]
                        .as_float()
                        .or_else(|_| arr[1].as_i64().map(|v| v as f32))
                        .ok()?;
                    let x1 = arr[2]
                        .as_float()
                        .or_else(|_| arr[2].as_i64().map(|v| v as f32))
                        .ok()?;
                    let y1 = arr[3]
                        .as_float()
                        .or_else(|_| arr[3].as_i64().map(|v| v as f32))
                        .ok()?;
                    let w = (x1 - x0).abs();
                    let h = (y1 - y0).abs();
                    let name = if (w - 595.0).abs() < 10.0 && (h - 842.0).abs() < 10.0 {
                        "A4"
                    } else if (w - 612.0).abs() < 10.0 && (h - 792.0).abs() < 10.0 {
                        "US Letter"
                    } else if (w - 612.0).abs() < 10.0 && (h - 1008.0).abs() < 10.0 {
                        "US Legal"
                    } else {
                        "Custom"
                    };
                    Some(format!(
                        "{name} ({:.0} × {:.0} pt / {:.1} × {:.1} mm)",
                        w,
                        h,
                        w * 0.352778,
                        h * 0.352778
                    ))
                } else {
                    None
                }
            });

            let image_count = d
                .objects
                .values()
                .filter(|obj| {
                    if let Ok(dict) = obj.as_dict() {
                        if let Ok(subtype) = dict.get(b"Subtype").and_then(|o| o.as_name()) {
                            return subtype == b"Image";
                        }
                    }
                    false
                })
                .count() as u32;

            Ok(PdfInfoResult {
                is_encrypted,
                pages,
                file_size,
                title,
                author,
                subject,
                keywords,
                creator,
                producer,
                creation_date,
                mod_date,
                pdf_version,
                page_size,
                image_count,
            })
        }
        Err(e) => {
            let err_str = format!("{}", e);
            let is_encrypted = err_str.contains("encrypted") || err_str.contains("password");
            Ok(PdfInfoResult {
                is_encrypted,
                pages: 0,
                file_size,
                title: None,
                author: None,
                subject: None,
                keywords: None,
                creator: None,
                producer: None,
                creation_date: None,
                mod_date: None,
                pdf_version: None,
                page_size: None,
                image_count: 0,
            })
        }
    }
}

#[tauri::command]
pub fn sanitize_pdf(input_path: String, output_path: String) -> AppResult<()> {
    validate_path(&input_path)?;
    validate_output_path(&output_path)?;
    ensure_distinct_output(&output_path, &[input_path.as_str()])?;

    let mut doc = io::load_doc(&input_path)?;
    io::reject_encrypted(&doc, &input_path)?;
    let expected_pages = doc.get_pages().len();

    // 1. Remove Info dict completely from trailer
    doc.trailer.remove(b"Info");

    // 2. Sanitize Catalog (Root)
    if let Ok(catalog_ref) = doc.trailer.get(b"Root").and_then(|obj| obj.as_reference()) {
        if let Some(cat_obj) = doc.objects.get_mut(&catalog_ref) {
            if let Ok(cat_dict) = cat_obj.as_dict_mut() {
                cat_dict.remove(b"Metadata");
                cat_dict.remove(b"PieceInfo");
                cat_dict.remove(b"OpenAction");
                cat_dict.remove(b"AA");
                cat_dict.remove(b"Names");
            }
        }
    }

    // 3. Sanitize each page
    let pages = doc.get_pages();
    for page_id in pages.values() {
        if let Some(p_obj) = doc.objects.get_mut(page_id) {
            if let Ok(p_dict) = p_obj.as_dict_mut() {
                p_dict.remove(b"PieceInfo");
                p_dict.remove(b"AA");
            }
        }
    }

    // 4. Compress to scrub unused unreferenced objects
    doc.compress();

    let policy = ValidationPolicy {
        expected_pages: Some(expected_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &output_path, &policy)?;
    Ok(())
}

#[derive(Debug, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePdfInfo {
    pub local_path: String,
    pub file_name: String,
    pub file_size: u64,
}

#[tauri::command]
pub async fn download_pdf_from_url(url: String) -> AppResult<RemotePdfInfo> {
    let trimmed = url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err(AppError::Pdf(
            "URL must start with http:// or https://".into(),
        ));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| AppError::Pdf(format!("Failed to build HTTP client: {e}")))?;

    let resp: reqwest::Response = client
        .get(trimmed)
        .send()
        .await
        .map_err(|e| AppError::Pdf(format!("Failed to fetch URL: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::Pdf(format!(
            "HTTP error {}: {}",
            resp.status(),
            resp.status().canonical_reason().unwrap_or("Unknown")
        )));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::Pdf(format!("Failed to read response body: {e}")))?;

    if bytes.len() < 4 || &bytes[0..4] != b"%PDF" {
        return Err(AppError::Pdf(
            "The downloaded file does not appear to be a valid PDF document (missing %PDF header)"
                .into(),
        ));
    }

    let url_clean = trimmed.split('?').next().unwrap_or(trimmed);
    let mut file_name = url_clean
        .split('/')
        .last()
        .unwrap_or("remote_document.pdf")
        .to_string();
    if file_name.is_empty() {
        file_name = "remote_document.pdf".to_string();
    }
    if !file_name.to_lowercase().ends_with(".pdf") {
        file_name.push_str(".pdf");
    }

    let cache_dir = std::env::temp_dir().join("pdf_seeker_remote");
    std::fs::create_dir_all(&cache_dir).map_err(AppError::from)?;
    let unique_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let local_path = cache_dir.join(format!("{}_{}", unique_id, file_name));

    std::fs::write(&local_path, &bytes).map_err(AppError::from)?;

    Ok(RemotePdfInfo {
        local_path: local_path.to_string_lossy().to_string(),
        file_name,
        file_size: bytes.len() as u64,
    })
}
