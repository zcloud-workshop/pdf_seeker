use lopdf::{Document, Object, ObjectId};
use serde::Deserialize;

use crate::commands::validation::{validate_output_path, validate_path};
use crate::error::{AppError, AppResult};
use crate::pdf::io::{self, ensure_distinct_output, ValidationPolicy};

pub type ObjId = ObjectId;

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

pub fn get_pages_ref(doc: &Document) -> AppResult<ObjectId> {
    let root_ref = doc
        .trailer
        .get(b"Root")
        .and_then(|o| o.as_reference())
        .map_err(|e| AppError::Pdf(format!("Root error: {}", e)))?;
    doc.get_object(root_ref)
        .and_then(|o| o.as_dict())
        .and_then(|d| d.get(b"Pages"))
        .and_then(|o| o.as_reference())
        .map_err(|e| AppError::Pdf(format!("Pages error: {}", e)))
}

fn parse_page_ranges(ranges: &str, max: u32) -> AppResult<Vec<Vec<u32>>> {
    if ranges.trim().is_empty() {
        return Err(AppError::Pdf("Empty ranges".into()));
    }
    let mut result = Vec::new();
    for part in ranges.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.contains('-') {
            let nums: Vec<&str> = trimmed.split('-').collect();
            if nums.len() != 2 {
                return Err(AppError::Pdf(format!("Invalid range: {}", trimmed)));
            }
            let s: u32 = nums[0]
                .parse()
                .map_err(|_| AppError::Pdf(format!("Invalid number: {}", nums[0])))?;
            let e: u32 = nums[1]
                .parse()
                .map_err(|_| AppError::Pdf(format!("Invalid number: {}", nums[1])))?;
            if s < 1 || e > max || s > e {
                return Err(AppError::Pdf(format!(
                    "Range {} out of bounds (1-{})",
                    trimmed, max
                )));
            }
            result.push((s..=e).collect());
        } else {
            let n: u32 = trimmed
                .parse()
                .map_err(|_| AppError::Pdf(format!("Invalid number: {}", trimmed)))?;
            if n < 1 || n > max {
                return Err(AppError::Pdf(format!(
                    "Page {} out of bounds (1-{})",
                    n, max
                )));
            }
            result.push(vec![n]);
        }
    }
    if result.is_empty() {
        return Err(AppError::Pdf("No valid ranges".into()));
    }
    Ok(result)
}

fn load_for_write(path: &str) -> AppResult<Document> {
    let doc = io::load_doc(path)?;
    io::reject_encrypted(&doc, path)?;
    Ok(doc)
}

#[tauri::command]
pub fn merge_pdfs(paths: Vec<String>, output_path: String) -> AppResult<()> {
    if paths.is_empty() {
        return Err(AppError::Pdf("No input PDFs".into()));
    }
    for p in &paths {
        validate_path(p)?;
    }
    validate_output_path(&output_path)?;
    let inputs: Vec<&str> = paths.iter().map(String::as_str).collect();
    ensure_distinct_output(&output_path, &inputs)?;

    let mut merged = load_for_write(&paths[0])?;
    let mut total_expected_pages = merged.get_pages().len();

    for path in paths.iter().skip(1) {
        let mut doc = load_for_write(path)?;
        let doc_page_count = doc.get_pages().len();
        total_expected_pages += doc_page_count;

        let old_page_ids: Vec<ObjId> = doc.get_pages().values().copied().collect();
        let mut sorted_old_ids: Vec<ObjId> = doc.objects.keys().copied().collect();
        sorted_old_ids.sort();

        let start_id = merged.max_id + 1;
        doc.renumber_objects_with(start_id);

        let id_map: std::collections::BTreeMap<ObjId, ObjId> = sorted_old_ids
            .iter()
            .enumerate()
            .map(|(i, old)| (*old, (start_id + i as u32, 0)))
            .collect();

        let doc_pages: Vec<ObjId> = old_page_ids
            .iter()
            .map(|old| *id_map.get(old).unwrap_or(old))
            .collect();

        for (id, obj) in doc.objects {
            merged.objects.insert(id, obj);
        }
        if let Some(max_key) = merged.objects.keys().max() {
            merged.max_id = merged.max_id.max(max_key.0);
        }

        let root_ref = merged
            .trailer
            .get(b"Root")
            .and_then(|o| o.as_reference())
            .map_err(|e| AppError::Pdf(format!("Root error: {}", e)))?;

        let pages_ref = merged
            .get_object(root_ref)
            .and_then(|o| o.as_dict())
            .and_then(|d| d.get(b"Pages"))
            .and_then(|o| o.as_reference())
            .map_err(|e| AppError::Pdf(format!("Pages error: {}", e)))?;

        let merged_count = merged.get_pages().len();

        {
            let pages_obj = merged
                .objects
                .get_mut(&pages_ref)
                .ok_or_else(|| AppError::Pdf("Pages object missing".into()))?;

            let pages_dict = pages_obj
                .as_dict_mut()
                .map_err(|e| AppError::Pdf(format!("Pages dict error: {}", e)))?;

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

        // Ensure every merged page's Parent points to the root Pages object
        for page_id in &doc_pages {
            if let Ok(page_obj) = merged.get_object_mut(*page_id) {
                if let Ok(page_dict) = page_obj.as_dict_mut() {
                    page_dict.set("Parent", Object::Reference(pages_ref));
                }
            }
        }
    }

    let policy = ValidationPolicy {
        expected_pages: Some(total_expected_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut merged, &output_path, &policy)?;
    Ok(())
}

#[tauri::command]
pub fn rotate_pdf(req: RotatePdfRequest) -> AppResult<()> {
    validate_path(&req.input_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(&req.output_path, &[req.input_path.as_str()])?;

    let mut doc = load_for_write(&req.input_path)?;
    let expected_pages = doc.get_pages().len();
    let page_ids: Vec<ObjId> = doc.get_pages().values().copied().collect();

    for page_id in page_ids {
        if let Some(page_obj) = doc.objects.get_mut(&page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                let cur = dict
                    .get(b"Rotate")
                    .ok()
                    .and_then(|o| o.as_i64().ok())
                    .unwrap_or(0);

                let normalized_angle = ((cur + req.angle as i64) % 360 + 360) % 360;
                dict.set("Rotate", Object::Integer(normalized_angle));
            }
        }
    }

    let policy = ValidationPolicy {
        expected_pages: Some(expected_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;
    Ok(())
}

#[tauri::command]
pub fn delete_pages(req: DeletePagesRequest) -> AppResult<()> {
    validate_path(&req.input_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(&req.output_path, &[req.input_path.as_str()])?;

    let mut doc = load_for_write(&req.input_path)?;
    let total = doc.get_pages().len();

    let mut unique_pages = req.pages_to_delete.clone();
    unique_pages.sort_unstable();
    unique_pages.dedup();

    if unique_pages.is_empty() {
        return Err(AppError::Pdf("No pages specified to delete".into()));
    }
    for &p in &unique_pages {
        if p < 1 || (p as usize) > total {
            return Err(AppError::Pdf(format!(
                "Page index {} is out of bounds (1..={})",
                p, total
            )));
        }
    }
    if unique_pages.len() >= total {
        return Err(AppError::Pdf("Cannot delete all pages of a document".into()));
    }
    let expected = total.saturating_sub(unique_pages.len());

    doc.delete_pages(&unique_pages);
    if let Ok(pages_ref) = get_pages_ref(&doc) {
        if let Ok(pages_obj) = doc.get_object_mut(pages_ref) {
            if let Ok(pages_dict) = pages_obj.as_dict_mut() {
                pages_dict.set("Count", Object::Integer(expected as i64));
            }
        }
    }

    let policy = ValidationPolicy {
        expected_pages: Some(expected),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;
    Ok(())
}

#[tauri::command]
pub fn split_pdf(req: SplitPdfRequest) -> AppResult<Vec<String>> {
    validate_path(&req.input_path)?;
    validate_path(&req.output_dir)?;

    let doc = load_for_write(&req.input_path)?;
    let total = doc.get_pages().len() as u32;
    let mut output_paths = Vec::new();

    let ranges = if req.mode == "single" || req.mode == "all" {
        (1..=total).map(|p| vec![p]).collect::<Vec<_>>()
    } else {
        parse_page_ranges(&req.ranges.unwrap_or_default(), total)?
    };

    for range in &ranges {
        let mut doc_clone = doc.clone();
        let pages_to_delete: Vec<u32> = (1..=total).filter(|p| !range.contains(p)).collect();
        if !pages_to_delete.is_empty() {
            doc_clone.delete_pages(&pages_to_delete);
        }
        if let Ok(pages_ref) = get_pages_ref(&doc_clone) {
            if let Ok(pages_obj) = doc_clone.get_object_mut(pages_ref) {
                if let Ok(pages_dict) = pages_obj.as_dict_mut() {
                    pages_dict.set("Count", Object::Integer(range.len() as i64));
                }
            }
        }
        let name = if range.len() == 1 {
            format!("page_{}.pdf", range[0])
        } else {
            format!("pages_{}-{}.pdf", range[0], range[range.len() - 1])
        };
        let output_path = format!(
            "{}/{}",
            req.output_dir.trim_end_matches('/').trim_end_matches('\\'),
            name
        );
        ensure_distinct_output(&output_path, &[req.input_path.as_str()])?;

        let policy = ValidationPolicy {
            expected_pages: Some(range.len()),
            check_page_tree: true,
        };
        io::write_transactional(&mut doc_clone, &output_path, &policy)?;
        output_paths.push(output_path);
    }

    Ok(output_paths)
}

#[tauri::command]
pub fn extract_pages_pdf(req: ExtractPagesRequest) -> AppResult<()> {
    validate_path(&req.input_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(&req.output_path, &[req.input_path.as_str()])?;

    let mut doc = load_for_write(&req.input_path)?;
    let total = doc.get_pages().len() as u32;

    let mut unique_pages = req.pages_to_extract.clone();
    unique_pages.sort_unstable();
    unique_pages.dedup();

    if unique_pages.is_empty() {
        return Err(AppError::Pdf("No pages specified to extract".into()));
    }
    for &p in &unique_pages {
        if p < 1 || p > total {
            return Err(AppError::Pdf(format!(
                "Page index {} is out of bounds (1..={})",
                p, total
            )));
        }
    }

    let pages_to_delete: Vec<u32> = (1..=total)
        .filter(|p| !unique_pages.contains(p))
        .collect();
    if !pages_to_delete.is_empty() {
        doc.delete_pages(&pages_to_delete);
    }

    let policy = ValidationPolicy {
        expected_pages: Some(unique_pages.len()),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;
    Ok(())
}

#[tauri::command]
pub fn reorder_pages(req: ReorderPagesRequest) -> AppResult<()> {
    validate_path(&req.input_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(&req.output_path, &[req.input_path.as_str()])?;

    let mut doc = load_for_write(&req.input_path)?;
    let pages = doc.get_pages();
    let total = pages.len() as u32;

    if req.new_order.len() != total as usize {
        return Err(AppError::Pdf(format!(
            "Expected {} page numbers, got {}",
            total,
            req.new_order.len()
        )));
    }

    let mut current_order: Vec<(u32, ObjectId)> = pages.iter().map(|(n, id)| (*n, *id)).collect();
    current_order.sort_by_key(|(n, _)| *n);

    let mut new_kids = Vec::new();
    for page_num in &req.new_order {
        if *page_num < 1 || *page_num > total {
            return Err(AppError::Pdf(format!("Invalid page number: {}", page_num)));
        }
        let (_, page_id) = current_order
            .iter()
            .find(|(n, _)| *n == *page_num)
            .ok_or_else(|| AppError::Pdf(format!("Page {} not found", page_num)))?;
        new_kids.push(Object::Reference(*page_id));
    }

    let pages_ref = get_pages_ref(&doc)?;
    if let Some(pages_obj) = doc.objects.get_mut(&pages_ref) {
        if let Ok(dict) = pages_obj.as_dict_mut() {
            dict.set("Kids", Object::Array(new_kids));
        }
    }

    let policy = ValidationPolicy {
        expected_pages: Some(total as usize),
        check_page_tree: true,
    };
    io::write_transactional(&mut doc, &req.output_path, &policy)?;
    Ok(())
}

#[tauri::command]
pub fn insert_pages(req: InsertPagesRequest) -> AppResult<()> {
    validate_path(&req.input_path)?;
    validate_path(&req.source_path)?;
    validate_output_path(&req.output_path)?;
    ensure_distinct_output(
        &req.output_path,
        &[req.input_path.as_str(), req.source_path.as_str()],
    )?;

    let mut target = load_for_write(&req.input_path)?;
    let mut source = load_for_write(&req.source_path)?;

    let old_source_page_ids: Vec<ObjectId> = source.get_pages().values().copied().collect();
    let mut sorted_old_ids: Vec<ObjectId> = source.objects.keys().copied().collect();
    sorted_old_ids.sort();

    let start_id = target.max_id + 1;
    source.renumber_objects_with(start_id);

    let id_map: std::collections::BTreeMap<ObjectId, ObjectId> = sorted_old_ids
        .iter()
        .enumerate()
        .map(|(i, old)| (*old, (start_id + i as u32, 0)))
        .collect();

    let source_page_ids: Vec<ObjectId> = old_source_page_ids
        .iter()
        .map(|old| *id_map.get(old).unwrap_or(old))
        .collect();

    for (id, obj) in source.objects {
        target.objects.insert(id, obj);
    }
    if let Some(max_key) = target.objects.keys().max() {
        target.max_id = target.max_id.max(max_key.0);
    }

    let target_count = target.get_pages().len();
    let pos = req.insert_position as usize;
    if pos > target_count {
        return Err(AppError::Pdf(format!(
            "Insert position {} exceeds page count {}",
            pos, target_count
        )));
    }

    let pages_ref = get_pages_ref(&target)?;
    {
        if let Some(pages_obj) = target.objects.get_mut(&pages_ref) {
            if let Ok(dict) = pages_obj.as_dict_mut() {
                if let Ok(kids) = dict.get_mut(b"Kids") {
                    if let Ok(arr) = kids.as_array_mut() {
                        for (i, page_id) in source_page_ids.iter().enumerate() {
                            arr.insert(pos + i, Object::Reference(*page_id));
                        }
                    }
                }
                dict.set(
                    "Count",
                    Object::Integer((target_count + source_page_ids.len()) as i64),
                );
            }
        }
    }

    // Ensure every inserted page's Parent points to target's root Pages object
    for page_id in &source_page_ids {
        if let Ok(page_obj) = target.get_object_mut(*page_id) {
            if let Ok(page_dict) = page_obj.as_dict_mut() {
                page_dict.set("Parent", Object::Reference(pages_ref));
            }
        }
    }

    let expected_pages = target_count + source_page_ids.len();
    let policy = ValidationPolicy {
        expected_pages: Some(expected_pages),
        check_page_tree: true,
    };
    io::write_transactional(&mut target, &req.output_path, &policy)?;
    Ok(())
}

