use lopdf::{Document, Object};
use serde::{Deserialize, Serialize};

use crate::commands::annotate::embed_image;
use crate::commands::validation::{validate_output_path, validate_path};
use crate::error::{AppError, AppResult};
use crate::pdf::io::{self, ensure_distinct_output, ValidationPolicy};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagesToPdfRequest {
    pub image_paths: Vec<String>,
    pub output_path: String,
}

#[derive(Debug, Serialize)]
pub struct TextExtractResult {
    pub text: String,
    pub pages: usize,
}

#[tauri::command]
pub fn images_to_pdf(req: ImagesToPdfRequest) -> AppResult<()> {
    if req.image_paths.is_empty() {
        return Err(AppError::Pdf("No images provided".into()));
    }
    for p in &req.image_paths {
        validate_path(p)?;
    }
    validate_output_path(&req.output_path)?;
    let inputs: Vec<&str> = req.image_paths.iter().map(String::as_str).collect();
    ensure_distinct_output(&req.output_path, &inputs)?;

    let mut doc = Document::with_version("1.4");
    let catalog_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
    let pages_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
        (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
        (
            b"Count".to_vec(),
            Object::Integer(req.image_paths.len() as i64),
        ),
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
            .map_err(|e| AppError::Pdf(format!("Read '{}': {}", image_path, e)))?;
        let (image_id, w, h) = embed_image(&mut doc, &data, image_path)?;

        let content = format!("q {} 0 0 {} 0 0 cm /Im1 Do Q", w, h);
        let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            content.into_bytes(),
        )));
        let resources_id =
            doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![(
                b"XObject".to_vec(),
                Object::Dictionary(lopdf::Dictionary::from_iter(vec![(
                    b"Im1".to_vec(),
                    Object::Reference(image_id),
                )])),
            )])));
        let page_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Page".to_vec())),
            (b"Parent".to_vec(), Object::Reference(pages_id)),
            (
                b"MediaBox".to_vec(),
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(w as i64),
                    Object::Integer(h as i64),
                ]),
            ),
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

    let policy = ValidationPolicy {
        expected_pages: Some(req.image_paths.len()),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;
    Ok(())
}

#[tauri::command]
pub fn extract_text(path: String) -> AppResult<TextExtractResult> {
    validate_path(&path)?;
    let doc = io::load_doc(&path)?;
    io::reject_encrypted(&doc, &path)?;
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
