use image as img_crate;
use lopdf::{Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, MutexGuard, OnceLock};

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

static PDF_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(super) fn lock_pdf_writes() -> AppResult<MutexGuard<'static, ()>> {
    PDF_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "PDF write lock is unavailable".to_string())
}

fn load_doc(path: &str) -> AppResult<Document> {
    Document::load(path).map_err(|e| format!("Load '{}': {}", path, e))
}

struct TempPdfFile(std::path::PathBuf);

impl Drop for TempPdfFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn save_doc_with(
    doc: &mut Document,
    path: &str,
    write: impl FnOnce(&mut Document, &std::path::Path) -> std::io::Result<()>,
) -> AppResult<()> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);
    let target = std::path::Path::new(path);
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let file_name = target
        .file_name()
        .ok_or_else(|| format!("Invalid output path: {}", path))?;
    let mut temp = None;
    for _ in 0..32 {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            id
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => {
                drop(file);
                temp = Some(TempPdfFile(temp_path));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Create temporary PDF beside '{}': {}", path, e)),
        }
    }
    let temp = temp.ok_or_else(|| {
        format!(
            "Could not reserve temporary PDF beside '{}': name collisions",
            path
        )
    })?;
    write(doc, &temp.0).map_err(|e| format!("Save temporary PDF for '{}': {}", path, e))?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&temp.0)
        .map_err(|e| format!("Open temporary PDF for '{}': {}", path, e))?;
    file.sync_all()
        .map_err(|e| format!("Flush temporary PDF for '{}': {}", path, e))?;
    Document::load(&temp.0).map_err(|e| format!("Validate temporary PDF for '{}': {}", path, e))?;
    replace_pdf_file(&temp.0, target).map_err(|e| format!("Replace '{}': {}", path, e))?;
    if let Ok(dir) = std::fs::File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

pub(super) fn save_doc(doc: &mut Document, path: &str) -> AppResult<()> {
    save_doc_with(doc, path, |doc, temp| doc.save(temp).map(|_| ()))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfFileFingerprint {
    pub size: u64,
    pub modified_ms: i64,
}

fn fingerprint_of(metadata: &std::fs::Metadata) -> PdfFileFingerprint {
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default();
    PdfFileFingerprint {
        size: metadata.len(),
        modified_ms,
    }
}

#[tauri::command]
pub fn pdf_file_fingerprint(path: String) -> AppResult<PdfFileFingerprint> {
    let metadata =
        std::fs::metadata(&path).map_err(|e| format!("Read metadata of '{}': {}", path, e))?;
    Ok(fingerprint_of(&metadata))
}

/// Restores `path` to the bytes of `snapshot_path` through the shared atomic
/// save path. `expected_size`/`expected_modified_ms` come from the last state
/// the frontend observed; a mismatch means another process changed the file
/// and the restore is refused so the caller can reload.
#[tauri::command]
pub fn commit_pdf_snapshot(
    path: String,
    snapshot_path: String,
    expected_size: Option<u64>,
    expected_modified_ms: Option<i64>,
) -> AppResult<PdfFileFingerprint> {
    let _guard = lock_pdf_writes()?;
    if expected_size.is_some() || expected_modified_ms.is_some() {
        let metadata =
            std::fs::metadata(&path).map_err(|e| format!("Read metadata of '{}': {}", path, e))?;
        let current = fingerprint_of(&metadata);
        if let Some(size) = expected_size {
            if current.size != size {
                return Err(format!(
                    "'{}' was modified outside this app (size changed); reload it before retrying",
                    path
                ));
            }
        }
        if let Some(modified_ms) = expected_modified_ms {
            if current.modified_ms != modified_ms {
                return Err(format!(
                    "'{}' was modified outside this app (timestamp changed); reload it before retrying",
                    path
                ));
            }
        }
    }
    let mut doc = Document::load(&snapshot_path)
        .map_err(|e| format!("Load snapshot '{}': {}", snapshot_path, e))?;
    let result = save_doc(&mut doc, &path).and_then(|_| {
        let metadata =
            std::fs::metadata(&path).map_err(|e| format!("Read metadata of '{}': {}", path, e))?;
        Ok(fingerprint_of(&metadata))
    });
    // The caller's scratch file is only ever ours to delete inside the system
    // temp dir; arbitrary paths are left untouched.
    if std::path::Path::new(&snapshot_path).starts_with(std::env::temp_dir()) {
        let _ = std::fs::remove_file(&snapshot_path);
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn replace_pdf_file(temp: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    std::fs::rename(temp, target)
}

#[cfg(target_os = "windows")]
fn replace_pdf_file(temp: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
        fn GetLastError() -> u32;
    }

    let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let ok = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0x1 | 0x8) };
    if ok == 0 {
        let code = unsafe { GetLastError() };
        Err(std::io::Error::from_raw_os_error(code as i32))
    } else {
        Ok(())
    }
}

fn flatten_contents(
    doc: &Document,
    object: &Object,
    out: &mut Vec<ObjectId>,
    visited: &mut std::collections::HashSet<ObjectId>,
) -> AppResult<()> {
    match object {
        Object::Reference(id) => {
            if !visited.insert(*id) {
                return Err(format!("Circular Contents reference at {} {}", id.0, id.1));
            }
            let resolved = doc
                .get_object(*id)
                .map_err(|e| format!("Resolve Contents: {}", e))?;
            match resolved {
                Object::Array(items) => {
                    for item in items {
                        flatten_contents(doc, item, out, visited)?;
                    }
                }
                Object::Stream(_) => out.push(*id),
                _ => return Err("Contents must resolve to a stream or array".into()),
            }
            visited.remove(id);
        }
        Object::Array(items) => {
            for item in items {
                flatten_contents(doc, item, out, visited)?;
            }
        }
        Object::Stream(_) => {
            return Err("Direct Contents streams are unsupported by lopdf serialization".into())
        }
        _ => return Err("Contents must be a stream reference or array".into()),
    }
    Ok(())
}

fn append_page_content(
    doc: &mut Document,
    page_id: ObjectId,
    new_stream_id: ObjectId,
) -> AppResult<()> {
    let contents = doc
        .get_object(page_id)
        .map_err(|e| format!("Page error: {}", e))?
        .as_dict()
        .map_err(|e| format!("Page dictionary error: {}", e))?
        .get(b"Contents")
        .ok()
        .cloned();
    let mut streams = Vec::new();
    if let Some(contents) = contents {
        flatten_contents(
            doc,
            &contents,
            &mut streams,
            &mut std::collections::HashSet::new(),
        )?;
    }
    streams.push(new_stream_id);
    let page = doc
        .objects
        .get_mut(&page_id)
        .ok_or("Page object is missing")?;
    page.as_dict_mut()
        .map_err(|e| format!("Page dictionary error: {}", e))?
        .set(
            "Contents",
            Object::Array(streams.into_iter().map(Object::Reference).collect()),
        );
    Ok(())
}

fn add_page_resource(
    doc: &mut Document,
    page_id: ObjectId,
    category: &[u8],
    preferred: &[u8],
    value: Object,
) -> AppResult<Vec<u8>> {
    fn as_dict(doc: &Document, object: &Object) -> AppResult<lopdf::Dictionary> {
        match object {
            Object::Dictionary(dict) => Ok(dict.clone()),
            Object::Reference(id) => doc
                .get_object(*id)
                .and_then(Object::as_dict)
                .map(Clone::clone)
                .map_err(|e| format!("Resolve resource dictionary: {}", e)),
            _ => Err("Resource entry must be a dictionary".into()),
        }
    }

    let existing = inherited_page_value(doc, page_id, b"Resources")?;
    let mut resources = match existing.as_ref() {
        Some(object) => as_dict(doc, object)?,
        None => lopdf::Dictionary::new(),
    };
    let mut category_dict = match resources.get(category) {
        Ok(object) => as_dict(doc, object)?,
        Err(_) => lopdf::Dictionary::new(),
    };
    let mut name = preferred.to_vec();
    let mut suffix = 1u32;
    while category_dict.has(&name) {
        name = format!("{}{}", String::from_utf8_lossy(preferred), suffix).into_bytes();
        suffix += 1;
    }
    category_dict.set(name.clone(), value);
    resources.set(category.to_vec(), Object::Dictionary(category_dict));
    let resource_id = doc.add_object(Object::Dictionary(resources));
    let page = doc
        .objects
        .get_mut(&page_id)
        .ok_or("Page object is missing")?
        .as_dict_mut()
        .map_err(|e| format!("Page dictionary error: {}", e))?;
    page.set("Resources", Object::Reference(resource_id));
    Ok(name)
}

static OCR_TASK_DIRS: OnceLock<Mutex<std::collections::HashSet<std::path::PathBuf>>> =
    OnceLock::new();

fn ocr_task_dirs() -> &'static Mutex<std::collections::HashSet<std::path::PathBuf>> {
    OCR_TASK_DIRS.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

struct OwnedOcrDir(std::path::PathBuf);

impl Drop for OwnedOcrDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn claim_ocr_dir(path: &std::path::Path) -> AppResult<OwnedOcrDir> {
    let mut dirs = ocr_task_dirs()
        .lock()
        .map_err(|_| "OCR task registry is unavailable".to_string())?;
    if !dirs.remove(path) {
        return Err("OCR image directory is not owned by an active task".into());
    }
    Ok(OwnedOcrDir(path.to_path_buf()))
}

fn escape_pdf_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn validate_page_text(text: &str) -> AppResult<()> {
    if !text.is_ascii() {
        return Err(
            "Visible page text currently supports ASCII only; no licensed CJK font is configured"
                .into(),
        );
    }
    Ok(())
}

fn parse_page_ranges(ranges: &str, max: u32) -> AppResult<Vec<Vec<u32>>> {
    if ranges.trim().is_empty() {
        return Err("Empty ranges".into());
    }
    let mut result = Vec::new();
    for part in ranges.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            return Err("Empty page range".into());
        }
        if trimmed.contains('-') {
            let nums: Vec<&str> = trimmed.split('-').collect();
            if nums.len() != 2 {
                return Err(format!("Invalid range: {}", trimmed));
            }
            let s: u32 = nums[0]
                .parse()
                .map_err(|_| format!("Invalid number: {}", nums[0]))?;
            let e: u32 = nums[1]
                .parse()
                .map_err(|_| format!("Invalid number: {}", nums[1]))?;
            if s < 1 || e > max || s > e {
                return Err(format!("Range {} out of bounds (1-{})", trimmed, max));
            }
            result.push((s..=e).collect());
        } else {
            let n: u32 = trimmed
                .parse()
                .map_err(|_| format!("Invalid number: {}", trimmed))?;
            if n < 1 || n > max {
                return Err(format!("Page {} out of bounds (1-{})", n, max));
            }
            result.push(vec![n]);
        }
    }
    if result.is_empty() {
        return Err("No valid ranges".into());
    }
    Ok(result)
}

fn embed_image(doc: &mut Document, data: &[u8], path: &str) -> AppResult<(ObjectId, u32, u32)> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "jpg" || ext == "jpeg" {
        let img = img_crate::load_from_memory(data).map_err(|e| format!("Image decode: {}", e))?;
        let (w, h) = (img.width(), img.height());
        let color_space = if img.color() == img_crate::ColorType::L8 {
            b"DeviceGray".to_vec()
        } else {
            b"DeviceRGB".to_vec()
        };
        let dict = lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"XObject".to_vec())),
            (b"Subtype".to_vec(), Object::Name(b"Image".to_vec())),
            (b"Width".to_vec(), Object::Integer(w as i64)),
            (b"Height".to_vec(), Object::Integer(h as i64)),
            (b"ColorSpace".to_vec(), Object::Name(color_space)),
            (b"BitsPerComponent".to_vec(), Object::Integer(8)),
            (b"Filter".to_vec(), Object::Name(b"DCTDecode".to_vec())),
        ]);
        let id = doc.add_object(Object::Stream(lopdf::Stream::new(dict, data.to_vec())));
        Ok((id, w, h))
    } else {
        let img = img_crate::load_from_memory(data).map_err(|e| format!("Image decode: {}", e))?;
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
        ]);
        let mut rgb_stream = lopdf::Stream::new(dict, rgb_data);
        rgb_stream
            .compress()
            .map_err(|e| format!("Compress image pixels: {}", e))?;
        let id = doc.add_object(Object::Stream(rgb_stream));
        if alpha_data.iter().any(|alpha| *alpha != 255) {
            let smask_dict = lopdf::Dictionary::from_iter(vec![
                (b"Type".to_vec(), Object::Name(b"XObject".to_vec())),
                (b"Subtype".to_vec(), Object::Name(b"Image".to_vec())),
                (b"Width".to_vec(), Object::Integer(w as i64)),
                (b"Height".to_vec(), Object::Integer(h as i64)),
                (b"ColorSpace".to_vec(), Object::Name(b"DeviceGray".to_vec())),
                (b"BitsPerComponent".to_vec(), Object::Integer(8)),
            ]);
            let mut alpha_stream = lopdf::Stream::new(smask_dict, alpha_data);
            alpha_stream
                .compress()
                .map_err(|e| format!("Compress image alpha: {}", e))?;
            let smask_id = doc.add_object(Object::Stream(alpha_stream));
            if let Some(obj) = doc.objects.get_mut(&id) {
                if let Ok(stream) = obj.as_stream_mut() {
                    stream.dict.set(b"SMask", Object::Reference(smask_id));
                }
            }
        }
        Ok((id, w, h))
    }
}

fn get_pages_ref(doc: &Document) -> AppResult<ObjectId> {
    let root_ref = doc
        .trailer
        .get(b"Root")
        .and_then(|o| o.as_reference())
        .map_err(|e| format!("Root error: {}", e))?;
    doc.get_object(root_ref)
        .and_then(|o| o.as_dict())
        .and_then(|d| d.get(b"Pages"))
        .and_then(|o| o.as_reference())
        .map_err(|e| format!("Pages error: {}", e))
}

fn inherited_page_value(
    doc: &Document,
    page_id: ObjectId,
    key: &[u8],
) -> AppResult<Option<Object>> {
    let mut current = page_id;
    let mut visited = std::collections::HashSet::new();
    loop {
        if !visited.insert(current) {
            return Err("Circular page Parent chain".into());
        }
        let dict = doc
            .get_object(current)
            .map_err(|e| format!("Page tree object error: {}", e))?
            .as_dict()
            .map_err(|e| format!("Page tree dictionary error: {}", e))?;
        if let Ok(value) = dict.get(key) {
            return Ok(Some(value.clone()));
        }
        match dict.get(b"Parent").and_then(Object::as_reference) {
            Ok(parent) => current = parent,
            Err(_) => return Ok(None),
        }
    }
}

fn flatten_page_tree(doc: &mut Document, page_ids: &[ObjectId]) -> AppResult<ObjectId> {
    let pages_ref = get_pages_ref(doc)?;
    fn collect_nodes(
        doc: &Document,
        node_id: ObjectId,
        out: &mut Vec<ObjectId>,
        visited: &mut std::collections::HashSet<ObjectId>,
    ) -> AppResult<()> {
        if !visited.insert(node_id) {
            return Err("Circular page tree".into());
        }
        out.push(node_id);
        let node = doc
            .get_object(node_id)
            .map_err(|e| format!("Page tree node error: {}", e))?;
        let dict = node
            .as_dict()
            .map_err(|e| format!("Page tree node dictionary error: {}", e))?;
        if dict.get(b"Type").and_then(Object::as_name).ok() != Some(b"Pages") {
            return Ok(());
        }
        let kids = dict
            .get(b"Kids")
            .and_then(Object::as_array)
            .map_err(|e| format!("Page tree Kids error: {}", e))?;
        for child in kids {
            let child_id = child
                .as_reference()
                .map_err(|e| format!("Page tree child error: {}", e))?;
            let child_dict = doc
                .get_object(child_id)
                .and_then(Object::as_dict)
                .map_err(|e| format!("Page tree child error: {}", e))?;
            if child_dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"Pages") {
                collect_nodes(doc, child_id, out, visited)?;
            }
        }
        Ok(())
    }
    let mut old_nodes = Vec::new();
    collect_nodes(
        doc,
        pages_ref,
        &mut old_nodes,
        &mut std::collections::HashSet::new(),
    )?;
    let inherited_keys: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];
    let mut inherited_by_page = Vec::with_capacity(page_ids.len());
    for page_id in page_ids {
        let mut values = Vec::new();
        for key in inherited_keys {
            let has_value = doc
                .get_object(*page_id)
                .map_err(|e| format!("Page error: {}", e))?
                .as_dict()
                .map_err(|e| format!("Page dictionary error: {}", e))?
                .get(key)
                .is_ok();
            if !has_value {
                if let Some(value) = inherited_page_value(doc, *page_id, key)? {
                    values.push((key.to_vec(), value));
                }
            }
        }
        inherited_by_page.push(values);
    }
    for (page_id, values) in page_ids.iter().zip(inherited_by_page) {
        let dict = doc
            .objects
            .get_mut(page_id)
            .ok_or("Page object is missing")?
            .as_dict_mut()
            .map_err(|e| format!("Page dictionary error: {}", e))?;
        for (key, value) in values {
            dict.set(key, value);
        }
        dict.set("Parent", Object::Reference(pages_ref));
    }
    let pages = doc
        .objects
        .get_mut(&pages_ref)
        .ok_or("Pages object is missing")?
        .as_dict_mut()
        .map_err(|e| format!("Pages dictionary error: {}", e))?;
    pages.set(
        "Kids",
        Object::Array(page_ids.iter().copied().map(Object::Reference).collect()),
    );
    pages.set("Count", Object::Integer(page_ids.len() as i64));
    for node_id in old_nodes.into_iter().filter(|id| *id != pages_ref) {
        doc.objects.remove(&node_id);
    }
    Ok(pages_ref)
}

fn get_page_size(page_dict: &lopdf::Dictionary) -> (f64, f64) {
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

fn obj_as_f64(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

#[tauri::command]
pub fn merge_pdfs(paths: Vec<String>, output_path: String) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
    if paths.is_empty() {
        return Err("No input PDFs".into());
    }

    let mut merged = load_doc(&paths[0])?;
    let first_pages: Vec<ObjectId> = merged.get_pages().values().copied().collect();
    flatten_page_tree(&mut merged, &first_pages)?;

    for path in paths.iter().skip(1) {
        let mut doc = load_doc(path)?;

        // Collect page IDs and all object IDs BEFORE renumbering
        let old_page_ids: Vec<ObjId> = doc.get_pages().values().copied().collect();
        flatten_page_tree(&mut doc, &old_page_ids)?;
        // Renumber so IDs don't collide with merged's objects
        let start_id = merged.max_id + 1;
        doc.renumber_objects_with(start_id);

        // Map old page IDs to new IDs
        let doc_pages: Vec<ObjId> = doc.get_pages().values().copied().collect();
        let source_pages_ref = get_pages_ref(&doc)?;
        let source_catalog_ref = doc
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .map_err(|e| format!("Source catalog error: {}", e))?;

        for (id, obj) in doc.objects {
            if id != source_pages_ref && id != source_catalog_ref {
                merged.objects.insert(id, obj);
            }
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

        let existing_kids = merged
            .objects
            .get(&pages_ref)
            .ok_or("Pages object missing")?
            .as_dict()
            .and_then(|dict| dict.get(b"Kids"))
            .and_then(Object::as_array)
            .map_err(|e| format!("Pages Kids error: {}", e))?
            .to_vec();
        let mut kids = existing_kids;
        for page_id in &doc_pages {
            let page = merged
                .objects
                .get_mut(page_id)
                .ok_or("Merged page object missing")?
                .as_dict_mut()
                .map_err(|e| format!("Merged page dictionary error: {}", e))?;
            page.set("Parent", Object::Reference(pages_ref));
            kids.push(Object::Reference(*page_id));
        }
        let pages_dict = merged
            .objects
            .get_mut(&pages_ref)
            .ok_or("Pages object missing")?
            .as_dict_mut()
            .map_err(|e| format!("Pages dict error: {}", e))?;
        pages_dict.set("Kids", Object::Array(kids));

        let total_count = (merged_count + doc_pages.len()) as i64;
        pages_dict.set("Count", Object::Integer(total_count));
    }

    save_doc(&mut merged, &output_path)
}

#[tauri::command]
pub fn rotate_pdf(req: RotatePdfRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
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

                dict.set("Rotate", Object::Integer((cur + req.angle as i64) % 360));
            }
        }
    }

    save_doc(&mut doc, &req.output_path)
}

#[tauri::command]
pub fn delete_pages(req: DeletePagesRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
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

/// Per-page text extraction (for PDF compare)
#[tauri::command]
pub fn extract_page_texts(path: String) -> AppResult<Vec<String>> {
    let doc = load_doc(&path)?;
    let pages = doc.get_pages();
    let mut texts = Vec::with_capacity(pages.len());
    for (page_num, _) in pages.iter() {
        let text = doc
            .extract_text(&[*page_num])
            .map_err(|e| format!("Extract page {}: {}", page_num, e))?;
        texts.push(text);
    }
    Ok(texts)
}

// ==================== Split PDF ====================
#[tauri::command]
pub fn split_pdf(req: SplitPdfRequest) -> AppResult<Vec<String>> {
    let _write_lock = lock_pdf_writes()?;
    let doc = load_doc(&req.input_path)?;
    let total = doc.get_pages().len() as u32;
    if total == 0 {
        return Err("PDF has no pages".into());
    }
    let ranges = if req.mode == "single" {
        (1..=total).map(|p| vec![p]).collect::<Vec<_>>()
    } else {
        parse_page_ranges(&req.ranges.unwrap_or_default(), total)?
    };

    let mut selected = std::collections::HashSet::new();
    for page in ranges.iter().flatten() {
        if !selected.insert(*page) {
            return Err(format!(
                "Page {} appears more than once in the split ranges",
                page
            ));
        }
    }
    let output_paths: Vec<String> = ranges
        .iter()
        .map(|range| {
            let name = if range.len() == 1 {
                format!("page_{}.pdf", range[0])
            } else {
                format!("pages_{}-{}.pdf", range[0], range[range.len() - 1])
            };
            std::path::Path::new(&req.output_dir)
                .join(name)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    if let Some(path) = output_paths
        .iter()
        .find(|path| std::path::Path::new(path).exists())
    {
        return Err(format!("Output already exists: {}", path));
    }

    let mut written = Vec::new();
    for (range, output_path) in ranges.iter().zip(&output_paths) {
        let mut doc_clone = doc.clone();
        let pages_to_delete: Vec<u32> = (1..=total).filter(|p| !range.contains(p)).collect();
        if !pages_to_delete.is_empty() {
            doc_clone.delete_pages(&pages_to_delete);
        }
        if let Err(error) = save_doc(&mut doc_clone, output_path) {
            for path in &written {
                let _ = std::fs::remove_file(path);
            }
            return Err(format!(
                "Split failed; earlier outputs were removed: {}",
                error
            ));
        }
        written.push(output_path);
    }

    Ok(output_paths)
}

// ==================== Extract Pages ====================

#[tauri::command]
pub fn extract_pages_pdf(req: ExtractPagesRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
    let mut doc = load_doc(&req.input_path)?;
    let total = doc.get_pages().len() as u32;
    if req.pages_to_extract.is_empty() {
        return Err("No pages selected".into());
    }
    let mut seen = std::collections::HashSet::new();
    for page in &req.pages_to_extract {
        if *page == 0 || *page > total {
            return Err(format!("Page {} is outside 1-{}", page, total));
        }
        if !seen.insert(*page) {
            return Err(format!("Duplicate page number: {}", page));
        }
    }
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
    let _write_lock = lock_pdf_writes()?;
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

    Ok(CompressResult {
        original_size,
        compressed_size,
        ratio,
    })
}

// ==================== Text Watermark ====================

#[tauri::command]
pub fn add_text_watermark(req: WatermarkRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
    validate_page_text(&req.text)?;
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
        (
            b"Encoding".to_vec(),
            Object::Name(b"WinAnsiEncoding".to_vec()),
        ),
    ])));

    for (_, page_id) in pages.iter() {
        let page = doc
            .get_object(*page_id)
            .map_err(|e| format!("Page error: {}", e))?;
        let page_dict = page
            .as_dict()
            .map_err(|e| format!("Page dict error: {}", e))?;
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
        let font_name = add_page_resource(
            &mut doc,
            *page_id,
            b"Font",
            b"F1",
            Object::Reference(font_id),
        )?;
        let gs_name = add_page_resource(
            &mut doc,
            *page_id,
            b"ExtGState",
            b"GS1",
            Object::Reference(gs_id),
        )?;

        let neg_sin = -sin_a;
        let watermark_bytes = format!(
            "q /{gs_name} gs BT /{font_name} {fs:.1} Tf {cos:.4} {sin:.4} {neg_sin:.4} {cos:.4} {cx:.1} {cy:.1} Tm {r:.3} {g:.3} {b:.3} rg ({escaped}) Tj ET Q",
            fs = req.font_size, cos = cos_a, sin = sin_a, neg_sin = neg_sin, cx = cx, cy = cy,
            r = r as f64 / 255.0, g = g as f64 / 255.0, b = b as f64 / 255.0,
            escaped = escaped, font_name = String::from_utf8_lossy(&font_name),
            gs_name = String::from_utf8_lossy(&gs_name)
        ).into_bytes();

        let watermark_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            watermark_bytes,
        )));

        append_page_content(&mut doc, *page_id, watermark_id)?;
    }

    save_doc(&mut doc, &req.output_path)
}

// ==================== Images to PDF ====================

#[tauri::command]
pub fn images_to_pdf(req: ImagesToPdfRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
    if req.image_paths.is_empty() {
        return Err("No images provided".into());
    }

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
        let data =
            std::fs::read(image_path).map_err(|e| format!("Read '{}': {}", image_path, e))?;
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

    save_doc(&mut doc, &req.output_path)
}

// ==================== Reorder Pages ====================

#[tauri::command]
pub fn reorder_pages(req: ReorderPagesRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
    let mut doc = load_doc(&req.input_path)?;
    let pages = doc.get_pages();
    let total = pages.len() as u32;

    if req.new_order.len() != total as usize {
        return Err(format!(
            "Expected {} page numbers, got {}",
            total,
            req.new_order.len()
        ));
    }

    let mut current_order: Vec<(u32, ObjectId)> = pages.iter().map(|(n, id)| (*n, *id)).collect();
    current_order.sort_by_key(|(n, _)| *n);

    let mut reordered_ids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for page_num in &req.new_order {
        if *page_num < 1 || *page_num > total {
            return Err(format!("Invalid page number: {}", page_num));
        }
        if !seen.insert(*page_num) {
            return Err(format!("Duplicate page number: {}", page_num));
        }
        let (_, page_id) = current_order
            .iter()
            .find(|(n, _)| *n == *page_num)
            .ok_or(format!("Page {} not found", page_num))?;
        reordered_ids.push(*page_id);
    }
    flatten_page_tree(&mut doc, &reordered_ids)?;

    save_doc(&mut doc, &req.output_path)
}

// ==================== Insert Pages ====================

#[tauri::command]
pub fn insert_pages(req: InsertPagesRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
    let mut target = load_doc(&req.input_path)?;
    let mut source = load_doc(&req.source_path)?;

    let old_source_page_ids: Vec<ObjectId> = source.get_pages().values().copied().collect();
    flatten_page_tree(&mut source, &old_source_page_ids)?;
    let start_id = target.max_id + 1;
    source.renumber_objects_with(start_id);

    let source_page_ids: Vec<ObjectId> = source.get_pages().values().copied().collect();
    let inherited_keys: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];
    let mut source_page_inherited = Vec::with_capacity(source_page_ids.len());
    for page_id in &source_page_ids {
        let mut values = Vec::new();
        for key in inherited_keys {
            if let Some(value) = inherited_page_value(&source, *page_id, key)? {
                values.push((key.to_vec(), value));
            }
        }
        source_page_inherited.push(values);
    }

    let source_pages_ref = get_pages_ref(&source)?;
    let source_catalog_ref = source
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(|e| format!("Source catalog error: {}", e))?;
    for (id, obj) in source.objects {
        if id != source_pages_ref && id != source_catalog_ref {
            target.objects.insert(id, obj);
        }
    }
    if let Some(max_key) = target.objects.keys().max() {
        target.max_id = target.max_id.max(max_key.0);
    }

    let mut target_page_ids: Vec<ObjectId> = target.get_pages().values().copied().collect();
    let target_count = target_page_ids.len();
    let pos = req.insert_position as usize;
    if pos > target_count {
        return Err(format!(
            "Insert position {} exceeds page count {}",
            pos, target_count
        ));
    }

    let pages_ref = flatten_page_tree(&mut target, &target_page_ids)?;
    for (page_id, values) in source_page_ids.iter().zip(source_page_inherited) {
        let page = target
            .objects
            .get_mut(page_id)
            .ok_or("Inserted page object missing")?
            .as_dict_mut()
            .map_err(|e| format!("Inserted page dictionary error: {}", e))?;
        for (key, value) in values {
            page.set(key, value);
        }
        page.set("Parent", Object::Reference(pages_ref));
    }
    target_page_ids.splice(pos..pos, source_page_ids);
    flatten_page_tree(&mut target, &target_page_ids)?;

    save_doc(&mut target, &req.output_path)
}

// ==================== Sign PDF (visual) ====================

#[tauri::command]
pub fn sign_pdf(req: SignPdfRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
    let mut doc = load_doc(&req.input_path)?;

    let sig_data =
        std::fs::read(&req.signature_image_path).map_err(|e| format!("Read signature: {}", e))?;
    let (image_id, _w, _h) = embed_image(&mut doc, &sig_data, &req.signature_image_path)?;

    let pages = doc.get_pages();
    let page_id = pages
        .get(&req.page)
        .ok_or(format!("Page {} not found", req.page))?;

    let image_name = add_page_resource(
        &mut doc,
        *page_id,
        b"XObject",
        b"SigImg",
        Object::Reference(image_id),
    )?;
    let content = format!(
        "q {} 0 0 {} {} {} cm /{} Do Q",
        req.width,
        req.height,
        req.x,
        req.y,
        String::from_utf8_lossy(&image_name)
    );
    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
        lopdf::Dictionary::new(),
        content.into_bytes(),
    )));

    append_page_content(&mut doc, *page_id, content_id)?;

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
    let task_dir = claim_ocr_dir(std::path::Path::new(&req.image_dir))?;
    if !check_tesseract_available() {
        return Err("Tesseract OCR is not installed. Please install it from https://github.com/tesseract-ocr/tesseract".into());
    }

    let mut full_text = String::new();
    let mut page_count = 0usize;

    let mut entries: Vec<_> = std::fs::read_dir(&task_dir.0)
        .map_err(|e| format!("Read dir '{}': {}", req.image_dir, e))?
        .map(|entry| entry.map_err(|e| format!("Read OCR task entry: {}", e)))
        .collect::<AppResult<Vec<_>>>()?;
    entries.retain(|e| {
        e.path()
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("png"))
            .unwrap_or(false)
    });
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

        if !output.status.success() {
            return Err(format!(
                "Tesseract failed for '{}': {}",
                path.display(),
                output.status
            ));
        }
        page_count += 1;
        let text = String::from_utf8_lossy(&output.stdout);
        full_text.push_str(&format!("\n--- Page {} ---\n", page_count));
        full_text.push_str(&text);
        full_text.push('\n');
    }

    if page_count == 0 {
        return Err("No text could be extracted from the images".into());
    }

    Ok(TextExtractResult {
        text: full_text,
        pages: page_count,
    })
}

// ==================== Temp Directory ====================

#[tauri::command]
pub fn get_temp_dir() -> AppResult<String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(0);
    let mut dirs = ocr_task_dirs()
        .lock()
        .map_err(|_| "OCR task registry is unavailable".to_string())?;
    let root = std::env::temp_dir().join("pdf_seeker_ocr");
    std::fs::create_dir_all(&root).map_err(|e| format!("Create OCR temp root: {}", e))?;
    let mut created = None;
    for _ in 0..32 {
        let id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
        let dir = root.join(format!("task_{}_{}", std::process::id(), id));
        match std::fs::create_dir(&dir) {
            Ok(()) => {
                created = Some(dir);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Create OCR task directory: {}", e)),
        }
    }
    let dir = created.ok_or("Could not create a unique OCR task directory")?;
    dirs.insert(dir.clone());
    Ok(dir.to_string_lossy().to_string())
}

// ==================== Save Image File (bypasses fs plugin) ====================

#[tauri::command]
pub fn save_image_file(path: String, data: Vec<u8>) -> AppResult<()> {
    let target = std::path::Path::new(&path);
    let parent = target
        .parent()
        .ok_or("Image path must be inside an OCR task directory")?;
    let dirs = ocr_task_dirs()
        .lock()
        .map_err(|_| "OCR task registry is unavailable".to_string())?;
    if !dirs.contains(parent) {
        return Err("Image path is not inside an active OCR task directory".into());
    }
    std::fs::write(&path, &data).map_err(|e| format!("Write '{}': {}", path, e))
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
    let _write_lock = lock_pdf_writes()?;
    validate_page_text(&req.text)?;
    let mut doc = load_doc(&req.input_path)?;
    let pages = doc.get_pages();
    let page_id = pages
        .get(&req.page)
        .ok_or(format!("Page {} not found", req.page))?;

    let hex = req.color.trim_start_matches('#');
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
    let font_name = add_page_resource(
        &mut doc,
        *page_id,
        b"Font",
        b"F1",
        Object::Reference(font_id),
    )?;

    let escaped = escape_pdf_string(&req.text);
    let content = format!(
        "q BT /{font_name} {fs:.1} Tf {r:.3} {g:.3} {b:.3} rg {x:.1} {y:.1} Td ({escaped}) Tj ET Q",
        fs = req.font_size,
        r = r as f64 / 255.0,
        g = g as f64 / 255.0,
        b = b as f64 / 255.0,
        x = req.x,
        y = req.y,
        escaped = escaped,
        font_name = String::from_utf8_lossy(&font_name)
    )
    .into_bytes();

    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
        lopdf::Dictionary::new(),
        content,
    )));

    append_page_content(&mut doc, *page_id, content_id)?;

    save_doc(&mut doc, &req.output_path)
}

#[tauri::command]
pub fn add_rectangle(req: AddRectangleRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
    let mut doc = load_doc(&req.input_path)?;
    let pages = doc.get_pages();
    let page_id = pages
        .get(&req.page)
        .ok_or(format!("Page {} not found", req.page))?;

    let hex = req.border_color.trim_start_matches('#');
    let br = u8::from_str_radix(&hex.get(0..2).unwrap_or("00"), 16).unwrap_or(0);
    let bg = u8::from_str_radix(&hex.get(2..4).unwrap_or("00"), 16).unwrap_or(0);
    let bb = u8::from_str_radix(&hex.get(4..6).unwrap_or("00"), 16).unwrap_or(0);

    let mut content = format!(
        "q {bw:.1} w {br:.3} {bg:.3} {bb:.3} RG ",
        bw = req.border_width,
        br = br as f64 / 255.0,
        bg = bg as f64 / 255.0,
        bb = bb as f64 / 255.0
    );

    if let Some(ref fill) = req.fill_color {
        let fh = fill.trim_start_matches('#');
        let fr = u8::from_str_radix(&fh.get(0..2).unwrap_or("00"), 16).unwrap_or(0);
        let fg = u8::from_str_radix(&fh.get(2..4).unwrap_or("00"), 16).unwrap_or(0);
        let fb = u8::from_str_radix(&fh.get(4..6).unwrap_or("00"), 16).unwrap_or(0);
        content.push_str(&format!(
            "{fr:.3} {fg:.3} {fb:.3} rg ",
            fr = fr as f64 / 255.0,
            fg = fg as f64 / 255.0,
            fb = fb as f64 / 255.0
        ));
        content.push_str(&format!(
            "{} {} {} {} re B",
            req.x, req.y, req.width, req.height
        ));
    } else {
        content.push_str(&format!(
            "{} {} {} {} re S",
            req.x, req.y, req.width, req.height
        ));
    }
    content.push_str(" Q");

    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
        lopdf::Dictionary::new(),
        content.into_bytes(),
    )));

    append_page_content(&mut doc, *page_id, content_id)?;

    save_doc(&mut doc, &req.output_path)
}

#[tauri::command]
pub fn add_highlight(req: AddHighlightRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
    let mut doc = load_doc(&req.input_path)?;
    let pages = doc.get_pages();
    let page_id = pages
        .get(&req.page)
        .ok_or(format!("Page {} not found", req.page))?;

    let hex = req.color.trim_start_matches('#');
    let r = u8::from_str_radix(&hex.get(0..2).unwrap_or("ff"), 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex.get(2..4).unwrap_or("ff"), 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex.get(4..6).unwrap_or("00"), 16).unwrap_or(0);

    // Graphics state for transparency
    let gs_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
        (b"Type".to_vec(), Object::Name(b"ExtGState".to_vec())),
        (
            b"ca".to_vec(),
            Object::Real(req.opacity.min(1.0).max(0.0) as f32),
        ),
    ])));

    let rx = req.x;
    let ry = req.y;
    let rw = req.width;
    let rh = req.height;
    let gs_name = add_page_resource(
        &mut doc,
        *page_id,
        b"ExtGState",
        b"GS1",
        Object::Reference(gs_id),
    )?;
    let content = format!(
        "q /{gs_name} gs {r:.3} {g:.3} {b:.3} rg {rx} {ry} {rw} {rh} re f Q",
        r = r as f64 / 255.0,
        g = g as f64 / 255.0,
        b = b as f64 / 255.0,
        rx = rx,
        ry = ry,
        rw = rw,
        rh = rh,
        gs_name = String::from_utf8_lossy(&gs_name)
    )
    .into_bytes();

    let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
        lopdf::Dictionary::new(),
        content,
    )));

    append_page_content(&mut doc, *page_id, content_id)?;

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
    let _write_lock = lock_pdf_writes()?;
    if req.pages.is_empty() {
        return Err("No pages selected".into());
    }
    if req.width <= 0.0 || req.height <= 0.0 {
        return Err("Crop area must be non-empty".into());
    }

    let mut doc = load_doc(&req.input_path)?;
    let all_pages = doc.get_pages();

    for &page_num in &req.pages {
        let page_id = all_pages
            .get(&page_num)
            .ok_or(format!("Page {} not found", page_num))?;

        // MediaBox of the page (page-level value; fall back to the default
        // used by get_page_size when inherited)
        let media = {
            let page = doc
                .get_object(*page_id)
                .map_err(|e| format!("Page error: {}", e))?;
            let page_dict = page
                .as_dict()
                .map_err(|e| format!("Page dict error: {}", e))?;
            page_dict
                .get(b"MediaBox")
                .ok()
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
            return Err(format!(
                "Crop area for page {} is outside the MediaBox",
                page_num
            ));
        }

        let page_obj = doc.objects.get_mut(page_id).unwrap();
        let dict = page_obj
            .as_dict_mut()
            .map_err(|e| format!("Page dict error: {}", e))?;
        dict.set(
            "CropBox",
            Object::Array(vec![
                Object::Real(cx0 as f32),
                Object::Real(cy0 as f32),
                Object::Real(cx1 as f32),
                Object::Real(cy1 as f32),
            ]),
        );
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
    let _write_lock = lock_pdf_writes()?;
    let mut doc = load_doc(&req.input_path)?;
    let pages = doc.get_pages();
    let page_id = *pages
        .get(&req.page)
        .ok_or(format!("Page {} not found", req.page))?;

    let hex = req.color.trim_start_matches('#');
    let r = u8::from_str_radix(&hex.get(0..2).unwrap_or("ff"), 16).unwrap_or(255) as f64 / 255.0;
    let g = u8::from_str_radix(&hex.get(2..4).unwrap_or("ff"), 16).unwrap_or(255) as f64 / 255.0;
    let b = u8::from_str_radix(&hex.get(4..6).unwrap_or("00"), 16).unwrap_or(0) as f64 / 255.0;

    let (subtype, ap_id) = match req.annot_type.as_str() {
        "highlight" => {
            let gs_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                (b"Type".to_vec(), Object::Name(b"ExtGState".to_vec())),
                (
                    b"ca".to_vec(),
                    Object::Real(req.opacity.min(1.0).max(0.0) as f32),
                ),
            ])));
            let content = format!(
                "q /GS0 gs {r:.3} {g:.3} {b:.3} rg 0 0 {w:.1} {h:.1} re f Q",
                r = r,
                g = g,
                b = b,
                w = req.width,
                h = req.height
            );
            let res = lopdf::Dictionary::from_iter(vec![(
                b"ExtGState".to_vec(),
                Object::Dictionary(lopdf::Dictionary::from_iter(vec![(
                    b"GS0".to_vec(),
                    Object::Reference(gs_id),
                )])),
            )]);
            let ap_dict = lopdf::Dictionary::from_iter(vec![
                (
                    b"BBox".to_vec(),
                    Object::Array(vec![
                        Object::Integer(0),
                        Object::Integer(0),
                        Object::Real(req.width as f32),
                        Object::Real(req.height as f32),
                    ]),
                ),
                (b"Resources".to_vec(), Object::Dictionary(res)),
            ]);
            let id = doc.add_object(Object::Stream(lopdf::Stream::new(
                ap_dict,
                content.into_bytes(),
            )));
            (b"Highlight".to_vec(), Some(id))
        }
        "underline" => {
            let content = format!(
                "q {r:.3} {g:.3} {b:.3} RG 1.5 w 0 1 m {w:.1} 1 l S Q",
                r = r,
                g = g,
                b = b,
                w = req.width
            );
            let ap_dict = lopdf::Dictionary::from_iter(vec![
                (
                    b"BBox".to_vec(),
                    Object::Array(vec![
                        Object::Integer(0),
                        Object::Integer(0),
                        Object::Real(req.width as f32),
                        Object::Real(req.height.max(3.0) as f32),
                    ]),
                ),
                (
                    b"Resources".to_vec(),
                    Object::Dictionary(lopdf::Dictionary::new()),
                ),
            ]);
            let id = doc.add_object(Object::Stream(lopdf::Stream::new(
                ap_dict,
                content.into_bytes(),
            )));
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
        (
            b"Rect".to_vec(),
            Object::Array(vec![
                Object::Real(req.x as f32),
                Object::Real(req.y as f32),
                Object::Real((req.x + req.width) as f32),
                Object::Real((req.y + req.height) as f32),
            ]),
        ),
        (
            b"C".to_vec(),
            Object::Array(vec![
                Object::Real(r as f32),
                Object::Real(g as f32),
                Object::Real(b as f32),
            ]),
        ),
        (b"F".to_vec(), Object::Integer(4)),
        (
            b"T".to_vec(),
            Object::String(b"PDF Seeker".to_vec(), lopdf::StringFormat::Literal),
        ),
    ]);
    if !req.content.is_empty() {
        annot.set(
            b"Contents",
            Object::String(encode_pdf_text(&req.content), lopdf::StringFormat::Literal),
        );
    }
    if let Some(id) = ap_id {
        annot.set(
            b"AP",
            Object::Dictionary(lopdf::Dictionary::from_iter(vec![(
                b"N".to_vec(),
                Object::Reference(id),
            )])),
        );
    }

    let annot_id = doc.add_object(Object::Dictionary(annot));

    // Append to the page /Annots array (create or extend, inline or referenced)
    let existing_annots: Option<Object> = {
        let page = doc
            .get_object(page_id)
            .map_err(|e| format!("Page error: {}", e))?;
        let page_dict = page
            .as_dict()
            .map_err(|e| format!("Page dict error: {}", e))?;
        match page_dict.get(b"Annots") {
            Ok(Object::Reference(r)) => doc
                .get_object(*r)
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
        Object::String(bytes, _) if bytes.starts_with(&[0xFE, 0xFF]) => {
            let units: Vec<u16> = bytes[2..]
                .chunks_exact(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        }
        Object::String(bytes, _) if bytes.starts_with(&[0xFF, 0xFE]) => {
            let units: Vec<u16> = bytes[2..]
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        }
        Object::String(bytes, _) => String::from_utf8_lossy(bytes).to_string(),
        Object::Name(n) => String::from_utf8_lossy(n).to_string(),
        Object::Integer(i) => i.to_string(),
        Object::Real(r) => r.to_string(),
        _ => String::new(),
    }
}

fn field_type_from(field_type: Option<&[u8]>, flags: i64) -> &'static str {
    match field_type {
        Some(b"Tx") => "text",
        Some(b"Btn") => {
            if flags & 0x8000 != 0 {
                "radio"
            }
            // bit 16: radio
            else if flags & 0x10000 != 0 {
                "button"
            }
            // bit 17: pushbutton
            else {
                "checkbox"
            }
        }
        Some(b"Ch") => "choice",
        Some(b"Sig") => "signature",
        _ => "unknown",
    }
}

fn field_options(field_dict: &lopdf::Dictionary) -> Vec<String> {
    field_dict
        .get(b"Opt")
        .ok()
        .and_then(|o| o.as_array().ok())
        .map(|arr| {
            arr.iter()
                .filter_map(|opt| {
                    match opt {
                        // Choice options may be [export_value, label] pairs
                        Object::Array(pair) => pair.first().map(object_to_display_string),
                        other => Some(object_to_display_string(other)),
                    }
                })
                .collect()
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
    inherited_type: Option<Vec<u8>>,
    inherited_flags: i64,
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
        let field_type = dict
            .get(b"FT")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(Vec::from)
            .or_else(|| inherited_type.clone());
        let flags = dict
            .get(b"Ff")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(inherited_flags);

        let own_name = dict
            .get(b"T")
            .ok()
            .map(object_to_display_string)
            .unwrap_or_default();
        let qualified = if prefix.is_empty() {
            own_name.clone()
        } else {
            format!("{}.{}", prefix, own_name)
        };

        // Sub-fields: kids that themselves have /T
        let has_field_kids = dict
            .get(b"Kids")
            .ok()
            .and_then(|o| o.as_array().ok())
            .map(|kids| {
                kids.iter().any(|k| {
                    let kd = match k {
                        Object::Reference(r) => {
                            doc.get_object(*r).ok().and_then(|o| o.as_dict().ok())
                        }
                        Object::Dictionary(d) => Some(d),
                        _ => None,
                    };
                    kd.map(|d| d.get(b"T").is_ok()).unwrap_or(false)
                })
            })
            .unwrap_or(false);

        if has_field_kids {
            let kids = dict
                .get(b"Kids")
                .ok()
                .and_then(|o| o.as_array().ok())
                .cloned()
                .unwrap_or_default();
            walk_fields(doc, &kids, &qualified, field_type, flags, page_lookup, out);
            continue;
        }

        if field_type.is_none() {
            continue; // not a terminal field
        }

        let page_index = match dict.get(b"P").ok().and_then(|o| o.as_reference().ok()) {
            Some(p) => page_lookup.get(&p).copied().unwrap_or(0),
            None => 0,
        };

        out.push(FormField {
            name: qualified,
            field_type: field_type_from(field_type.as_deref(), flags).to_string(),
            value: dict
                .get(b"V")
                .ok()
                .map(object_to_display_string)
                .unwrap_or_default(),
            options: field_options(dict),
            page_index,
        });
    }
}

#[tauri::command]
pub fn get_form_fields(path: String) -> AppResult<Vec<FormField>> {
    let doc = load_doc(&path)?;
    let (_, acroform) =
        get_acroform(&doc).ok_or("This PDF has no AcroForm (no fillable form fields)")?;

    let mut page_lookup = std::collections::HashMap::new();
    for (num, id) in doc.get_pages() {
        page_lookup.insert(id, num);
    }

    let entries = acroform
        .get(b"Fields")
        .ok()
        .and_then(|o| o.as_array().ok())
        .cloned()
        .ok_or("AcroForm has no /Fields array")?;

    let mut fields = Vec::new();
    walk_fields(&doc, &entries, "", None, 0, &page_lookup, &mut fields);
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
    inherited_type: Option<Vec<u8>>,
    inherited_flags: i64,
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
        let field_type = dict
            .get(b"FT")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(Vec::from)
            .or_else(|| inherited_type.clone());
        let flags = dict
            .get(b"Ff")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(inherited_flags);

        let own_name = dict
            .get(b"T")
            .ok()
            .map(object_to_display_string)
            .unwrap_or_default();
        let qualified = if prefix.is_empty() {
            own_name.clone()
        } else {
            format!("{}.{}", prefix, own_name)
        };

        let has_field_kids = dict
            .get(b"Kids")
            .ok()
            .and_then(|o| o.as_array().ok())
            .map(|kids| {
                kids.iter().any(|k| {
                    let kd = match k {
                        Object::Reference(r) => {
                            doc.get_object(*r).ok().and_then(|o| o.as_dict().ok())
                        }
                        Object::Dictionary(d) => Some(d),
                        _ => None,
                    };
                    kd.map(|d| d.get(b"T").is_ok()).unwrap_or(false)
                })
            })
            .unwrap_or(false);

        if has_field_kids {
            let kids = dict
                .get(b"Kids")
                .ok()
                .and_then(|o| o.as_array().ok())
                .cloned()
                .unwrap_or_default();
            locate_terminal_fields(doc, &kids, &qualified, field_type, flags, out);
        } else if field_type.is_some() {
            out.push((
                qualified,
                obj_id,
                field_type_from(field_type.as_deref(), flags).to_string(),
            ));
        }
    }
}

fn promote_field_entries(
    doc: &mut Document,
    entries: &mut [Object],
    visited: &mut std::collections::HashSet<ObjectId>,
) -> AppResult<()> {
    for entry in entries {
        match entry {
            Object::Reference(id) => {
                if !visited.insert(*id) {
                    continue;
                }
                let mut dict = doc
                    .get_object(*id)
                    .and_then(Object::as_dict)
                    .map(Clone::clone)
                    .map_err(|e| format!("Field dictionary error: {}", e))?;
                if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
                    let mut kids = kids.clone();
                    promote_field_entries(doc, &mut kids, visited)?;
                    dict.set("Kids", Object::Array(kids));
                }
                doc.objects.insert(*id, Object::Dictionary(dict));
            }
            Object::Dictionary(dict) => {
                let mut dict = dict.clone();
                if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
                    let mut kids = kids.clone();
                    promote_field_entries(doc, &mut kids, visited)?;
                    dict.set("Kids", Object::Array(kids));
                }
                *entry = Object::Reference(doc.add_object(Object::Dictionary(dict)));
            }
            _ => return Err("AcroForm field entries must be dictionaries or references".into()),
        }
    }
    Ok(())
}

fn normalize_acroform_fields(doc: &mut Document, root_id: ObjectId) -> AppResult<Vec<Object>> {
    let root_dict = doc
        .get_object(root_id)
        .and_then(Object::as_dict)
        .map(Clone::clone)
        .map_err(|e| format!("Catalog error: {}", e))?;
    let acroform = root_dict
        .get(b"AcroForm")
        .map_err(|e| format!("AcroForm error: {}", e))?
        .clone();
    let (acroform_id, mut acroform_dict) = match acroform {
        Object::Reference(id) => (
            Some(id),
            doc.get_object(id)
                .and_then(Object::as_dict)
                .map(Clone::clone)
                .map_err(|e| format!("AcroForm error: {}", e))?,
        ),
        Object::Dictionary(dict) => (None, dict),
        _ => return Err("AcroForm must be a dictionary or reference".into()),
    };
    let mut entries = acroform_dict
        .get(b"Fields")
        .and_then(Object::as_array)
        .map(Clone::clone)
        .map_err(|e| format!("AcroForm Fields error: {}", e))?;
    promote_field_entries(doc, &mut entries, &mut std::collections::HashSet::new())?;
    acroform_dict.set("Fields", Object::Array(entries.clone()));
    match acroform_id {
        Some(id) => {
            doc.objects.insert(id, Object::Dictionary(acroform_dict));
        }
        None => {
            let catalog = doc
                .objects
                .get_mut(&root_id)
                .ok_or("Catalog object is missing")?
                .as_dict_mut()
                .map_err(|e| format!("Catalog error: {}", e))?;
            catalog.set("AcroForm", Object::Dictionary(acroform_dict));
        }
    }
    Ok(entries)
}

fn widget_appearance_states(doc: &Document, field_id: ObjectId) -> Vec<(ObjectId, Vec<u8>)> {
    let Some(kids) = doc
        .get_object(field_id)
        .ok()
        .and_then(|o| o.as_dict().ok())
        .and_then(|dict| dict.get(b"Kids").ok())
        .and_then(|o| o.as_array().ok())
    else {
        return Vec::new();
    };
    let mut states = Vec::new();
    for kid in kids {
        let Ok(widget_id) = kid.as_reference() else {
            continue;
        };
        let Some(widget) = doc
            .get_object(widget_id)
            .ok()
            .and_then(|o| o.as_dict().ok())
        else {
            continue;
        };
        let Some(appearance) = widget.get(b"AP").ok() else {
            continue;
        };
        let appearance = match appearance {
            Object::Reference(id) => doc.get_object(*id).ok(),
            object => Some(object),
        };
        let Some(normal) = appearance
            .and_then(|object| object.as_dict().ok())
            .and_then(|appearance| appearance.get(b"N").ok())
        else {
            continue;
        };
        let normal = match normal {
            Object::Reference(id) => doc.get_object(*id).ok(),
            object => Some(object),
        };
        let Some(Object::Dictionary(states_dict)) = normal else {
            continue;
        };
        for (name, _) in states_dict.iter() {
            if name.as_slice() != b"Off" {
                states.push((widget_id, name.clone()));
            }
        }
    }
    states
}

#[tauri::command]
pub fn fill_form(req: FillFormRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
    let mut doc = load_doc(&req.input_path)?;
    let (root_ref, _) =
        get_acroform(&doc).ok_or("This PDF has no AcroForm (no fillable form fields)")?;

    let entries = normalize_acroform_fields(&mut doc, root_ref)?;

    let mut located = Vec::new();
    locate_terminal_fields(&doc, &entries, "", None, 0, &mut located);

    let truthy = |v: &str| {
        matches!(
            v.to_lowercase().as_str(),
            "true" | "1" | "yes" | "on" | "checked"
        )
    };

    let mut applied = 0;
    for value in &req.values {
        let (_, obj_id, ftype) = match located.iter().find(|(name, _, _)| *name == value.name) {
            Some(l) => l,
            None => return Err(format!("Form field '{}' not found", value.name)),
        };
        let obj_id = obj_id.ok_or(format!(
            "Form field '{}' is not a reference and cannot be filled",
            value.name
        ))?;
        let widget_states = if ftype == "checkbox" || ftype == "radio" {
            widget_appearance_states(&doc, obj_id)
        } else {
            Vec::new()
        };
        let button_state = match ftype.as_str() {
            "checkbox" if truthy(&value.value) => widget_states
                .first()
                .map(|(_, state)| state.clone())
                .unwrap_or_else(|| b"Yes".to_vec()),
            "checkbox" => b"Off".to_vec(),
            "radio" => {
                let selected = value.value.as_bytes();
                if !widget_states.is_empty()
                    && !widget_states.iter().any(|(_, state)| state == selected)
                {
                    return Err(format!(
                        "Radio value '{}' is not an appearance state for field '{}'",
                        value.value, value.name
                    ));
                }
                selected.to_vec()
            }
            _ => Vec::new(),
        };
        {
            let field_obj = doc
                .objects
                .get_mut(&obj_id)
                .ok_or("Form field object is missing")?;
            let dict = field_obj
                .as_dict_mut()
                .map_err(|e| format!("Field dict error: {}", e))?;
            match ftype.as_str() {
                "text" | "choice" => dict.set(
                    b"V",
                    Object::String(encode_pdf_text(&value.value), lopdf::StringFormat::Literal),
                ),
                "checkbox" | "radio" => {
                    dict.set(b"V", Object::Name(button_state.clone()));
                    dict.set(b"AS", Object::Name(button_state.clone()));
                }
                other => {
                    return Err(format!(
                        "Field '{}' of type '{}' cannot be filled",
                        value.name, other
                    ))
                }
            }
        }
        let widget_ids: std::collections::HashSet<ObjectId> =
            widget_states.iter().map(|(id, _)| *id).collect();
        for widget_id in widget_ids {
            let available: Vec<Vec<u8>> = widget_states
                .iter()
                .filter(|(id, _)| *id == widget_id)
                .map(|(_, state)| state.clone())
                .collect();
            let state = if available.contains(&button_state) {
                button_state.clone()
            } else {
                b"Off".to_vec()
            };
            if let Some(Object::Dictionary(widget)) = doc.objects.get_mut(&widget_id) {
                widget.set(b"AS", Object::Name(state));
            }
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
        root_obj
            .as_dict()
            .unwrap()
            .get(b"AcroForm")
            .ok()
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextReplacement {
    pub page: u32,
    pub cover_x: f64,
    pub cover_y: f64,
    pub cover_width: f64,
    pub cover_height: f64,
    pub baseline_y: f64,
    pub font_size: f64,
    pub new_text: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceTextRequest {
    pub input_path: String,
    pub output_path: String,
    pub replacements: Vec<TextReplacement>,
}

/// Overlay-style text replacement: cover each original match with a white
/// rectangle and draw the replacement text at the original baseline. All
/// replacements on one page share a single appended content stream, so one
/// undo step reverts the whole operation.
#[tauri::command]
pub fn replace_text(req: ReplaceTextRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
    if req.replacements.is_empty() {
        return Err("No replacements given".into());
    }
    for replacement in &req.replacements {
        validate_page_text(&replacement.new_text)?;
    }

    let mut doc = load_doc(&req.input_path)?;
    let all_pages = doc.get_pages();

    // Group by page so each page gets a single appended stream
    let mut by_page: std::collections::BTreeMap<u32, Vec<&TextReplacement>> =
        std::collections::BTreeMap::new();
    for rep in &req.replacements {
        if rep.cover_width <= 0.0 || rep.cover_height <= 0.0 {
            return Err("Replacement cover area must be non-empty".into());
        }
        if rep.font_size <= 0.0 {
            return Err("Replacement font size must be positive".into());
        }
        if !all_pages.contains_key(&rep.page) {
            return Err(format!("Page {} not found", rep.page));
        }
        by_page.entry(rep.page).or_default().push(rep);
    }

    for (page_num, reps) in by_page {
        let page_id = *all_pages.get(&page_num).unwrap();

        let font_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Font".to_vec())),
            (b"Subtype".to_vec(), Object::Name(b"Type1".to_vec())),
            (b"BaseFont".to_vec(), Object::Name(b"Helvetica".to_vec())),
            (
                b"Encoding".to_vec(),
                Object::Name(b"WinAnsiEncoding".to_vec()),
            ),
        ])));
        let font_name = add_page_resource(
            &mut doc,
            page_id,
            b"Font",
            b"F1",
            Object::Reference(font_id),
        )?;

        // Cover rects: white fill
        let mut content = String::from("q\n1 1 1 rg\n");
        for rep in &reps {
            content.push_str(&format!(
                "{:.1} {:.1} {:.1} {:.1} re f\n",
                rep.cover_x, rep.cover_y, rep.cover_width, rep.cover_height
            ));
        }
        content.push_str("Q\n");

        // Replacement texts at original baselines (absolute Tm positioning)
        content.push_str("BT\n");
        for rep in &reps {
            let (r, g, b) = match &rep.color {
                Some(hex) => {
                    let h = hex.trim_start_matches('#');
                    let cr = u8::from_str_radix(&h.get(0..2).unwrap_or("00"), 16).unwrap_or(0)
                        as f64
                        / 255.0;
                    let cg = u8::from_str_radix(&h.get(2..4).unwrap_or("00"), 16).unwrap_or(0)
                        as f64
                        / 255.0;
                    let cb = u8::from_str_radix(&h.get(4..6).unwrap_or("00"), 16).unwrap_or(0)
                        as f64
                        / 255.0;
                    (cr, cg, cb)
                }
                None => (0.0, 0.0, 0.0),
            };
            content.push_str(&format!(
                "/{} {:.1} Tf {:.3} {:.3} {:.3} rg 1 0 0 1 {:.1} {:.1} Tm ({}) Tj\n",
                String::from_utf8_lossy(&font_name),
                rep.font_size,
                r,
                g,
                b,
                rep.cover_x,
                rep.baseline_y,
                escape_pdf_string(&rep.new_text)
            ));
        }
        content.push_str("ET");

        let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            content.into_bytes(),
        )));

        append_page_content(&mut doc, page_id, content_id)?;
    }

    save_doc(&mut doc, &req.output_path)
}

// ==================== Outline (bookmarks) ====================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineItemInput {
    pub title: String,
    pub page: u32,
    #[serde(default)]
    pub children: Vec<OutlineItemInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetOutlineRequest {
    pub input_path: String,
    pub output_path: String,
    pub items: Vec<OutlineItemInput>,
}

/// Encode a text string for a PDF string object: ASCII stays as-is,
/// anything else becomes UTF-16BE with a BOM (PDF 1.7 §7.9.2.2).
fn encode_pdf_text(s: &str) -> Vec<u8> {
    if s.is_ascii() {
        s.as_bytes().to_vec()
    } else {
        let mut bytes = vec![0xFE, 0xFF];
        for unit in s.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        bytes
    }
}

fn count_outline_items(items: &[OutlineItemInput]) -> i64 {
    items
        .iter()
        .map(|i| 1 + count_outline_items(&i.children))
        .sum()
}

fn can_replace_outline_losslessly(doc: &Document, outlines_id: ObjectId) -> bool {
    fn walk(
        doc: &Document,
        first: Option<ObjectId>,
        last: Option<ObjectId>,
        parent: ObjectId,
        pages: &std::collections::HashSet<ObjectId>,
        seen: &mut std::collections::HashSet<ObjectId>,
    ) -> bool {
        if first.is_none() != last.is_none() {
            return false;
        }
        let Some(mut current) = first else {
            return true;
        };
        let expected_last = last.unwrap();
        let mut previous = None;
        loop {
            if !seen.insert(current) {
                return false;
            }
            let Ok(Object::Dictionary(dict)) = doc.get_object(current) else {
                return false;
            };
            if dict.iter().any(|(key, _)| {
                !matches!(
                    key.as_slice(),
                    b"Title"
                        | b"Parent"
                        | b"Prev"
                        | b"Next"
                        | b"First"
                        | b"Last"
                        | b"Count"
                        | b"Dest"
                )
            }) {
                return false;
            }
            if !matches!(dict.get(b"Title"), Ok(Object::String(_, _)))
                || dict.get(b"Parent").and_then(Object::as_reference).ok() != Some(parent)
                || dict.get(b"Prev").and_then(Object::as_reference).ok() != previous
            {
                return false;
            }
            let Ok(Object::Array(destination)) = dict.get(b"Dest") else {
                return false;
            };
            if !destination
                .first()
                .and_then(|object| object.as_reference().ok())
                .map(|id| pages.contains(&id))
                .unwrap_or(false)
            {
                return false;
            }
            let child_first = dict.get(b"First").and_then(Object::as_reference).ok();
            let child_last = dict.get(b"Last").and_then(Object::as_reference).ok();
            if !walk(doc, child_first, child_last, current, pages, seen) {
                return false;
            }
            let next = dict.get(b"Next").and_then(Object::as_reference).ok();
            if next.is_none() {
                return current == expected_last;
            }
            previous = Some(current);
            current = next.unwrap();
        }
    }

    let Ok(Object::Dictionary(root)) = doc.get_object(outlines_id) else {
        return false;
    };
    if root
        .iter()
        .any(|(key, _)| !matches!(key.as_slice(), b"Type" | b"First" | b"Last" | b"Count"))
    {
        return false;
    }
    let first = root.get(b"First").and_then(Object::as_reference).ok();
    let last = root.get(b"Last").and_then(Object::as_reference).ok();
    let pages: std::collections::HashSet<ObjectId> = doc.get_pages().values().copied().collect();
    let mut seen = std::collections::HashSet::new();
    walk(doc, first, last, outlines_id, &pages, &mut seen)
}

/// Create outline item objects for one nesting level, link siblings,
/// recurse into children. Returns the object ids of this level.
fn build_outline_items(
    doc: &mut Document,
    pages: &std::collections::BTreeMap<u32, ObjectId>,
    items: &[OutlineItemInput],
    parent: ObjectId,
) -> Result<Vec<ObjectId>, String> {
    let mut ids = Vec::with_capacity(items.len());
    for item in items {
        if item.title.trim().is_empty() {
            return Err("Outline title cannot be empty".into());
        }
        let page_id = *pages
            .get(&item.page)
            .ok_or_else(|| format!("Page {} not found", item.page))?;
        let dict = lopdf::Dictionary::from_iter(vec![
            (
                b"Title".to_vec(),
                Object::String(encode_pdf_text(&item.title), lopdf::StringFormat::Literal),
            ),
            (b"Parent".to_vec(), Object::Reference(parent)),
            (
                b"Dest".to_vec(),
                Object::Array(vec![
                    Object::Reference(page_id),
                    Object::Name(b"XYZ".to_vec()),
                    Object::Null,
                    Object::Null,
                    Object::Null,
                ]),
            ),
        ]);
        ids.push(doc.add_object(Object::Dictionary(dict)));
    }

    for (i, id) in ids.iter().enumerate() {
        let child_ids = build_outline_items(doc, pages, &items[i].children, *id)?;
        if let Some(Object::Dictionary(dict)) = doc.objects.get_mut(id) {
            if i > 0 {
                dict.set(b"Prev", Object::Reference(ids[i - 1]));
            }
            if i + 1 < ids.len() {
                dict.set(b"Next", Object::Reference(ids[i + 1]));
            }
            if let Some(first) = child_ids.first() {
                dict.set(b"First", Object::Reference(*first));
                dict.set(b"Last", Object::Reference(*child_ids.last().unwrap()));
                dict.set(
                    b"Count",
                    Object::Integer(count_outline_items(&items[i].children)),
                );
            }
        }
    }
    Ok(ids)
}

#[tauri::command]
pub fn set_outline(req: SetOutlineRequest) -> AppResult<()> {
    let _write_lock = lock_pdf_writes()?;
    let mut doc = load_doc(&req.input_path)?;
    let catalog_id = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(|e| format!("Catalog error: {}", e))?;
    if let Ok(outlines) = doc
        .get_object(catalog_id)
        .and_then(Object::as_dict)
        .and_then(|dict| dict.get(b"Outlines"))
    {
        let outlines_id = outlines.as_reference().map_err(|_| {
            "This PDF has inline bookmarks that the current request cannot preserve"
        })?;
        if !can_replace_outline_losslessly(&doc, outlines_id) {
            return Err("This PDF has bookmarks that the current request cannot preserve".into());
        }
    }
    let pages = doc.get_pages();

    let root_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![(
        b"Type".to_vec(),
        Object::Name(b"Outlines".to_vec()),
    )])));

    let top_ids = build_outline_items(&mut doc, &pages, &req.items, root_id)?;

    if let Some(Object::Dictionary(dict)) = doc.objects.get_mut(&root_id) {
        if let Some(first) = top_ids.first() {
            dict.set(b"First", Object::Reference(*first));
            dict.set(b"Last", Object::Reference(*top_ids.last().unwrap()));
        }
        dict.set(b"Count", Object::Integer(count_outline_items(&req.items)));
    }

    let root_ref = doc
        .trailer
        .get(b"Root")
        .map_err(|e| format!("Catalog error: {}", e))?
        .as_reference()
        .map_err(|_| "Catalog is not a reference".to_string())?;
    if let Some(Object::Dictionary(dict)) = doc.objects.get_mut(&root_ref) {
        dict.set(b"Outlines", Object::Reference(root_id));
    } else {
        return Err("Catalog is not a dictionary".into());
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
                (
                    b"MediaBox".to_vec(),
                    Object::Array(vec![
                        Object::Integer(0),
                        Object::Integer(0),
                        Object::Integer(612),
                        Object::Integer(792),
                    ]),
                ),
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
    fn test_repeated_mixed_edits_flatten_contents() {
        let dir = TempDir::new().unwrap();
        let path =
            std::path::Path::new(&create_test_pdf(dir.path(), "contents.pdf", 1)).to_path_buf();
        let mut doc = Document::load(&path).unwrap();
        let page_id = *doc.get_pages().get(&1).unwrap();
        let base_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            b"BT /F1 12 Tf (Original text) Tj ET".to_vec(),
        )));
        let nested_id = doc.add_object(Object::Array(vec![Object::Reference(base_id)]));
        let direct_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            b"q 0 0 m 1 1 l S Q".to_vec(),
        )));
        doc.get_object_mut(page_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set(
                "Contents",
                Object::Array(vec![
                    Object::Reference(nested_id),
                    Object::Reference(direct_id),
                ]),
            );
        doc.save(&path).unwrap();
        let path = path.to_string_lossy().into_owned();

        add_text_to_page(AddTextRequest {
            input_path: path.clone(),
            output_path: path.clone(),
            text: "First".into(),
            page: 1,
            x: 10.0,
            y: 20.0,
            font_size: 12.0,
            color: "000000".into(),
        })
        .unwrap();
        add_highlight(AddHighlightRequest {
            input_path: path.clone(),
            output_path: path.clone(),
            page: 1,
            x: 10.0,
            y: 20.0,
            width: 80.0,
            height: 12.0,
            color: "ffff00".into(),
            opacity: 0.4,
        })
        .unwrap();
        add_rectangle(AddRectangleRequest {
            input_path: path.clone(),
            output_path: path.clone(),
            page: 1,
            x: 5.0,
            y: 5.0,
            width: 40.0,
            height: 30.0,
            border_color: "000000".into(),
            fill_color: None,
            border_width: 1.0,
        })
        .unwrap();

        let doc = Document::load(&path).unwrap();
        let page_id = *doc.get_pages().get(&1).unwrap();
        let contents = doc
            .get_object(page_id)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Contents")
            .unwrap();
        let streams = match contents {
            Object::Array(items) => items,
            other => panic!("Expected flat contents array, got {:?}", other),
        };
        assert_eq!(streams.len(), 5);
        let content_bytes: Vec<u8> = doc
            .get_page_contents(page_id)
            .iter()
            .flat_map(|id| {
                doc.get_object(*id)
                    .unwrap()
                    .as_stream()
                    .unwrap()
                    .content
                    .clone()
            })
            .collect();
        let content_text = String::from_utf8_lossy(&content_bytes);
        assert!(content_text.contains("Original text"));
        assert!(content_text.contains("First"));
        assert!(content_text.contains("re f"));
        assert!(content_text.contains(" re S"));
    }

    #[test]
    fn test_page_edit_copies_shared_resources_and_allocates_unique_names() {
        let dir = TempDir::new().unwrap();
        let path =
            std::path::Path::new(&create_test_pdf(dir.path(), "resources.pdf", 2)).to_path_buf();
        let mut doc = Document::load(&path).unwrap();
        let pages: Vec<ObjectId> = doc.get_pages().values().copied().collect();
        let font_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
        let xobject_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
        let gs_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
        let resources_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (
                b"Font".to_vec(),
                Object::Dictionary(lopdf::Dictionary::from_iter(vec![(
                    b"F1".to_vec(),
                    Object::Reference(font_id),
                )])),
            ),
            (
                b"XObject".to_vec(),
                Object::Dictionary(lopdf::Dictionary::from_iter(vec![(
                    b"Keep".to_vec(),
                    Object::Reference(xobject_id),
                )])),
            ),
            (
                b"ExtGState".to_vec(),
                Object::Dictionary(lopdf::Dictionary::from_iter(vec![(
                    b"GS1".to_vec(),
                    Object::Reference(gs_id),
                )])),
            ),
        ])));
        for page_id in &pages {
            doc.get_object_mut(*page_id)
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("Resources", Object::Reference(resources_id));
        }
        doc.save(&path).unwrap();
        let path = path.to_string_lossy().into_owned();

        add_text_to_page(AddTextRequest {
            input_path: path.clone(),
            output_path: path.clone(),
            text: "preserve".into(),
            page: 1,
            x: 10.0,
            y: 20.0,
            font_size: 12.0,
            color: "000000".into(),
        })
        .unwrap();
        add_highlight(AddHighlightRequest {
            input_path: path.clone(),
            output_path: path.clone(),
            page: 1,
            x: 10.0,
            y: 20.0,
            width: 80.0,
            height: 12.0,
            color: "ffff00".into(),
            opacity: 0.4,
        })
        .unwrap();

        let doc = Document::load(&path).unwrap();
        let pages: Vec<ObjectId> = doc.get_pages().values().copied().collect();
        let page_resources = doc
            .get_object(pages[0])
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Resources")
            .unwrap()
            .as_reference()
            .unwrap();
        assert_ne!(page_resources, resources_id);
        assert_eq!(
            doc.get_object(pages[1])
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"Resources")
                .unwrap()
                .as_reference()
                .unwrap(),
            resources_id
        );
        let resources = doc.get_object(page_resources).unwrap().as_dict().unwrap();
        let fonts = resources.get(b"Font").unwrap().as_dict().unwrap();
        assert!(fonts.has(b"F1"));
        assert!(fonts.has(b"F11"));
        assert!(resources
            .get(b"XObject")
            .unwrap()
            .as_dict()
            .unwrap()
            .has(b"Keep"));
        let states = resources.get(b"ExtGState").unwrap().as_dict().unwrap();
        assert!(states.has(b"GS1"));
        assert!(states.has(b"GS11"));
    }

    #[test]
    fn test_same_file_concurrent_edits_are_serialized() {
        let dir = TempDir::new().unwrap();
        let path = create_test_pdf(dir.path(), "concurrent.pdf", 1);
        let mut handles = Vec::new();
        for x in [10.0, 100.0] {
            let input_path = path.clone();
            let output_path = path.clone();
            handles.push(std::thread::spawn(move || {
                add_rectangle(AddRectangleRequest {
                    input_path,
                    output_path,
                    page: 1,
                    x,
                    y: 10.0,
                    width: 50.0,
                    height: 30.0,
                    border_color: "000000".into(),
                    fill_color: None,
                    border_width: 1.0,
                })
            }));
        }
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
        let doc = Document::load(&path).unwrap();
        let page_id = *doc.get_pages().get(&1).unwrap();
        assert_eq!(doc.get_page_contents(page_id).len(), 2);
    }

    #[test]
    fn test_r08_inherited_float_geometry_keeps_legacy_watermark_semantics() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "geometry.pdf", 1);
        let mut doc = Document::load(&src).unwrap();
        let page_id = *doc.get_pages().get(&1).unwrap();
        let pages_id = get_pages_ref(&doc).unwrap();
        doc.get_object_mut(page_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .remove(b"MediaBox");
        let pages = doc.get_object_mut(pages_id).unwrap().as_dict_mut().unwrap();
        pages.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(12.5),
                Object::Real(-7.25),
                Object::Real(612.75),
                Object::Real(784.5),
            ]),
        );
        pages.set(
            "CropBox",
            Object::Array(vec![
                Object::Real(20.0),
                Object::Real(30.0),
                Object::Real(500.0),
                Object::Real(700.0),
            ]),
        );
        pages.set("Rotate", Object::Integer(90));
        doc.save(&src).unwrap();

        let out = dir
            .path()
            .join("geometry-watermark.pdf")
            .to_string_lossy()
            .into_owned();
        add_text_watermark(WatermarkRequest {
            input_path: src,
            output_path: out.clone(),
            text: "sample".into(),
            font_size: 12.0,
            opacity: 0.5,
            angle: 0.0,
            color: "000000".into(),
        })
        .unwrap();
        let output = Document::load(out).unwrap();
        let page_id = *output.get_pages().get(&1).unwrap();
        let stream_id = *output.get_page_contents(page_id).last().unwrap();
        let stream = output.get_object(stream_id).unwrap().as_stream().unwrap();
        let content = String::from_utf8_lossy(&stream.content);
        assert!(content.contains("306.0 396.0 Tm"));
        assert!(output
            .get_object(page_id)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Rotate")
            .is_err());
        assert!(output
            .get_object(get_pages_ref(&output).unwrap())
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"CropBox")
            .is_ok());
    }

    #[test]
    fn test_image_stream_filters_match_encoded_data_and_gray_jpeg() {
        let mut doc = Document::with_version("1.4");
        let rgba = img_crate::RgbaImage::from_pixel(64, 64, img_crate::Rgba([20, 40, 60, 80]));
        let mut png = std::io::Cursor::new(Vec::new());
        img_crate::DynamicImage::ImageRgba8(rgba)
            .write_to(&mut png, img_crate::ImageFormat::Png)
            .unwrap();
        let (rgba_id, _, _) = embed_image(&mut doc, &png.into_inner(), "alpha.png").unwrap();
        let rgba_stream = doc.get_object(rgba_id).unwrap().as_stream().unwrap();
        assert_eq!(
            rgba_stream.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"FlateDecode"
        );
        assert!(rgba_stream
            .dict
            .get(b"SMask")
            .unwrap()
            .as_reference()
            .is_ok());
        let alpha_id = rgba_stream
            .dict
            .get(b"SMask")
            .unwrap()
            .as_reference()
            .unwrap();
        assert_eq!(
            doc.get_object(alpha_id)
                .unwrap()
                .as_stream()
                .unwrap()
                .dict
                .get(b"Filter")
                .unwrap()
                .as_name()
                .unwrap(),
            b"FlateDecode"
        );

        let gray = img_crate::GrayImage::from_pixel(64, 64, img_crate::Luma([128]));
        let mut jpeg = std::io::Cursor::new(Vec::new());
        img_crate::DynamicImage::ImageLuma8(gray)
            .write_to(&mut jpeg, img_crate::ImageFormat::Jpeg)
            .unwrap();
        let (gray_id, _, _) = embed_image(&mut doc, &jpeg.into_inner(), "gray.jpg").unwrap();
        let gray_stream = doc.get_object(gray_id).unwrap().as_stream().unwrap();
        assert_eq!(
            gray_stream
                .dict
                .get(b"ColorSpace")
                .unwrap()
                .as_name()
                .unwrap(),
            b"DeviceGray"
        );
        assert_eq!(
            gray_stream.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"DCTDecode"
        );
    }

    #[test]
    fn test_insert_pages_flattens_nested_tree_and_keeps_inherited_boxes() {
        let dir = TempDir::new().unwrap();
        let target_path =
            std::path::Path::new(&create_test_pdf(dir.path(), "nested.pdf", 3)).to_path_buf();
        let mut target = Document::load(&target_path).unwrap();
        let page_ids: Vec<ObjectId> = target.get_pages().values().copied().collect();
        let root_id = get_pages_ref(&target).unwrap();
        for page_id in &page_ids {
            target
                .get_object_mut(*page_id)
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .remove(b"MediaBox");
        }
        let group_a = target.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
            (b"Parent".to_vec(), Object::Reference(root_id)),
            (b"Count".to_vec(), Object::Integer(2)),
            (b"Kids".to_vec(), Object::Array(vec![])),
            (
                b"MediaBox".to_vec(),
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(200),
                    Object::Integer(300),
                ]),
            ),
        ])));
        let inner_a = target.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
            (b"Parent".to_vec(), Object::Reference(group_a)),
            (b"Count".to_vec(), Object::Integer(2)),
            (
                b"Kids".to_vec(),
                Object::Array(vec![
                    Object::Reference(page_ids[0]),
                    Object::Reference(page_ids[1]),
                ]),
            ),
        ])));
        target
            .get_object_mut(group_a)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Kids", Object::Array(vec![Object::Reference(inner_a)]));
        let group_b = target.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
            (b"Parent".to_vec(), Object::Reference(root_id)),
            (b"Count".to_vec(), Object::Integer(1)),
            (
                b"Kids".to_vec(),
                Object::Array(vec![Object::Reference(page_ids[2])]),
            ),
            (
                b"MediaBox".to_vec(),
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(300),
                    Object::Integer(400),
                ]),
            ),
        ])));
        for page_id in &page_ids[..2] {
            target
                .get_object_mut(*page_id)
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("Parent", Object::Reference(inner_a));
        }
        target
            .get_object_mut(page_ids[2])
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Parent", Object::Reference(group_b));
        let root = target
            .get_object_mut(root_id)
            .unwrap()
            .as_dict_mut()
            .unwrap();
        root.set(
            "Kids",
            Object::Array(vec![Object::Reference(group_a), Object::Reference(group_b)]),
        );
        root.set("Count", Object::Integer(3));
        target.save(&target_path).unwrap();
        let source_path = create_test_pdf(dir.path(), "insert-source.pdf", 1);
        let output_path = dir
            .path()
            .join("inserted.pdf")
            .to_string_lossy()
            .into_owned();

        insert_pages(InsertPagesRequest {
            input_path: target_path.to_string_lossy().into_owned(),
            source_path,
            output_path: output_path.clone(),
            insert_position: 1,
        })
        .unwrap();

        let output = Document::load(&output_path).unwrap();
        let pages: Vec<ObjectId> = output.get_pages().values().copied().collect();
        assert_eq!(pages.len(), 4);
        assert_eq!(pages[0], page_ids[0]);
        assert_eq!(pages[2], page_ids[1]);
        assert_eq!(pages[3], page_ids[2]);
        let root_id = get_pages_ref(&output).unwrap();
        let root = output.get_object(root_id).unwrap().as_dict().unwrap();
        assert_eq!(root.get(b"Count").unwrap().as_i64().unwrap(), 4);
        assert_eq!(root.get(b"Kids").unwrap().as_array().unwrap().len(), 4);
        for page_id in &pages {
            assert_eq!(
                output
                    .get_object(*page_id)
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"Parent")
                    .unwrap()
                    .as_reference()
                    .unwrap(),
                root_id
            );
        }
        let boxes: Vec<Vec<i64>> = pages
            .iter()
            .map(|id| {
                output
                    .get_object(*id)
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"MediaBox")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| item.as_i64().unwrap())
                    .collect()
            })
            .collect();
        assert_eq!(boxes[0][2], 200);
        assert_eq!(boxes[2][2], 200);
        assert_eq!(boxes[3][2], 300);
    }

    #[test]
    fn test_reorder_rejects_duplicate_without_changing_input() {
        let dir = TempDir::new().unwrap();
        let path = create_test_pdf(dir.path(), "reorder.pdf", 3);
        let before = std::fs::read(&path).unwrap();
        let result = reorder_pages(ReorderPagesRequest {
            input_path: path.clone(),
            output_path: path.clone(),
            new_order: vec![1, 1, 3],
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn test_atomic_pdf_write_failure_preserves_target() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("protected.pdf");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("original"), b"original bytes").unwrap();
        let mut doc = Document::with_version("1.4");
        let result = save_doc(&mut doc, path.to_str().unwrap());
        assert!(result.is_err());
        assert_eq!(
            std::fs::read(path.join("original")).unwrap(),
            b"original bytes"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn test_atomic_pdf_temporary_write_failure_preserves_target() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("write-failure.pdf");
        std::fs::write(&path, b"original pdf bytes").unwrap();
        let mut doc = Document::with_version("1.4");
        let result = save_doc_with(&mut doc, path.to_str().unwrap(), |_, temp| {
            std::fs::write(temp, b"partial pdf bytes")?;
            Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "injected write failure",
            ))
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original pdf bytes");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn test_commit_pdf_snapshot_restores_bytes_and_returns_fingerprint() {
        let dir = TempDir::new().unwrap();
        let path = create_test_pdf(dir.path(), "restore.pdf", 2);
        let snapshot = create_test_pdf(dir.path(), "snapshot.pdf", 5);
        let expected = pdf_file_fingerprint(path.clone()).unwrap();
        let result = commit_pdf_snapshot(
            path.clone(),
            snapshot,
            Some(expected.size),
            Some(expected.modified_ms),
        )
        .unwrap();
        assert_eq!(Document::load(&path).unwrap().get_pages().len(), 5);
        assert!(result.size > 0);
    }

    #[test]
    fn test_commit_pdf_snapshot_rejects_stale_fingerprint_without_touching_target() {
        let dir = TempDir::new().unwrap();
        let path = create_test_pdf(dir.path(), "stale.pdf", 2);
        let before = std::fs::read(&path).unwrap();
        let snapshot = create_test_pdf(dir.path(), "stale-snapshot.pdf", 1);
        let result = commit_pdf_snapshot(path.clone(), snapshot, Some(12345), None);
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn test_commit_pdf_snapshot_rejects_invalid_snapshot() {
        let dir = TempDir::new().unwrap();
        let path = create_test_pdf(dir.path(), "invalid-target.pdf", 1);
        let before = std::fs::read(&path).unwrap();
        let bad = dir.path().join("bad-snapshot.pdf");
        std::fs::write(&bad, b"not a pdf").unwrap();
        let result =
            commit_pdf_snapshot(path.clone(), bad.to_string_lossy().into_owned(), None, None);
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn test_generated_pdf_renders_with_independent_poppler() {
        if std::process::Command::new("pdftoppm")
            .arg("-v")
            .output()
            .is_err()
        {
            return;
        }
        let dir = TempDir::new().unwrap();
        let source = create_test_pdf(dir.path(), "poppler-source.pdf", 1);
        let pdf = dir
            .path()
            .join("poppler-output.pdf")
            .to_string_lossy()
            .into_owned();
        add_rectangle(AddRectangleRequest {
            input_path: source,
            output_path: pdf.clone(),
            page: 1,
            x: 40.0,
            y: 40.0,
            width: 120.0,
            height: 80.0,
            border_color: "000000".into(),
            fill_color: Some("ff0000".into()),
            border_width: 2.0,
        })
        .unwrap();

        let prefix = dir.path().join("rendered");
        let status = std::process::Command::new("pdftoppm")
            .args(["-f", "1", "-singlefile", "-png", "-r", "72"])
            .arg(&pdf)
            .arg(&prefix)
            .status()
            .unwrap();
        assert!(status.success());
        let image = img_crate::open(prefix.with_extension("png"))
            .unwrap()
            .to_rgb8();
        assert!(image
            .pixels()
            .any(|pixel| pixel[0] > 200 && pixel[1] < 80 && pixel[2] < 80));
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
    fn test_merge_uses_page_order_after_renumbering_generation_ids() {
        let dir = TempDir::new().unwrap();
        let target = create_test_pdf(dir.path(), "merge-target.pdf", 1);
        let source = create_test_pdf(dir.path(), "merge-source.pdf", 3);
        let mut doc = Document::load(&source).unwrap();
        let pages: Vec<ObjectId> = doc.get_pages().values().copied().collect();
        let root_id = get_pages_ref(&doc).unwrap();
        let mut reordered = Vec::new();
        for (index, old_id) in pages.iter().enumerate() {
            let new_id = (old_id.0, 2);
            let mut object = doc.objects.remove(old_id).unwrap();
            object.as_dict_mut().unwrap().set(
                "MediaBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer((100 * (index + 1)) as i64),
                    Object::Integer(500),
                ]),
            );
            doc.objects.insert(new_id, object);
            reordered.push(Object::Reference(new_id));
        }
        reordered.reverse();
        doc.get_object_mut(root_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Kids", Object::Array(reordered));
        doc.save(&source).unwrap();

        let output = dir
            .path()
            .join("merged-reversed.pdf")
            .to_string_lossy()
            .into_owned();
        merge_pdfs(vec![target, source], output.clone()).unwrap();
        let merged = Document::load(output).unwrap();
        let widths: Vec<i64> = merged
            .get_pages()
            .values()
            .map(|id| {
                merged
                    .get_object(*id)
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"MediaBox")
                    .unwrap()
                    .as_array()
                    .unwrap()[2]
                    .as_i64()
                    .unwrap()
            })
            .collect();
        assert_eq!(widths, vec![612, 300, 200, 100]);
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
        assert_eq!(
            annot.get(b"Subtype").unwrap().as_name().unwrap(),
            b"Highlight"
        );
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
            content: "检查这个".into(),
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

        let note = doc
            .get_object(annots[0].as_reference().unwrap())
            .unwrap()
            .as_dict()
            .unwrap();
        assert_eq!(note.get(b"Subtype").unwrap().as_name().unwrap(), b"Text");
        match note.get(b"Contents").unwrap() {
            Object::String(bytes, _) => assert_eq!(bytes, &encode_pdf_text("检查这个")),
            o => panic!("Unexpected Contents: {:?}", o),
        }

        let ul = doc
            .get_object(annots[1].as_reference().unwrap())
            .unwrap()
            .as_dict()
            .unwrap();
        assert_eq!(ul.get(b"Subtype").unwrap().as_name().unwrap(), b"Underline");
    }

    #[test]
    fn test_cjk_page_text_is_rejected_without_modifying_the_source() {
        let dir = TempDir::new().unwrap();
        let path = create_test_pdf(dir.path(), "cjk.pdf", 1);
        let before = std::fs::read(&path).unwrap();
        let result = add_text_to_page(AddTextRequest {
            input_path: path.clone(),
            output_path: path.clone(),
            text: "中文".into(),
            page: 1,
            x: 20.0,
            y: 20.0,
            font_size: 12.0,
            color: "000000".into(),
        });
        assert!(result.unwrap_err().contains("no licensed CJK font"));
        assert_eq!(std::fs::read(&path).unwrap(), before);
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
            (
                b"MediaBox".to_vec(),
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(612),
                    Object::Integer(792),
                ]),
            ),
        ])));
        if let Some(pages_obj) = doc.objects.get_mut(&pages_id) {
            if let Ok(d) = pages_obj.as_dict_mut() {
                d.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
            }
        }
        doc.trailer.set(b"Root", Object::Reference(catalog_id));

        let text_field_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"FT".to_vec(), Object::Name(b"Tx".to_vec())),
            (
                b"T".to_vec(),
                Object::String(b"fullname".to_vec(), lopdf::StringFormat::Literal),
            ),
            (
                b"V".to_vec(),
                Object::String(b"old value".to_vec(), lopdf::StringFormat::Literal),
            ),
            (b"P".to_vec(), Object::Reference(page_id)),
        ])));
        let checkbox_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"FT".to_vec(), Object::Name(b"Btn".to_vec())),
            (
                b"T".to_vec(),
                Object::String(b"subscribe".to_vec(), lopdf::StringFormat::Literal),
            ),
            (b"V".to_vec(), Object::Name(b"Off".to_vec())),
            (b"AS".to_vec(), Object::Name(b"Off".to_vec())),
            (b"P".to_vec(), Object::Reference(page_id)),
        ])));
        let radio_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"FT".to_vec(), Object::Name(b"Btn".to_vec())),
            (b"Ff".to_vec(), Object::Integer(0x8000)), // radio flag
            (
                b"T".to_vec(),
                Object::String(b"color".to_vec(), lopdf::StringFormat::Literal),
            ),
            (b"V".to_vec(), Object::Name(b"red".to_vec())),
            (
                b"Opt".to_vec(),
                Object::Array(vec![
                    Object::String(b"red".to_vec(), lopdf::StringFormat::Literal),
                    Object::String(b"green".to_vec(), lopdf::StringFormat::Literal),
                ]),
            ),
            (b"P".to_vec(), Object::Reference(page_id)),
        ])));

        let mut widget_ids = Vec::new();
        for (field_id, state) in [
            (checkbox_id, b"Subscribed".to_vec()),
            (radio_id, b"red".to_vec()),
            (radio_id, b"green".to_vec()),
        ] {
            let off_id = doc.add_object(Object::Stream(lopdf::Stream::new(
                lopdf::Dictionary::new(),
                Vec::new(),
            )));
            let on_id = doc.add_object(Object::Stream(lopdf::Stream::new(
                lopdf::Dictionary::new(),
                Vec::new(),
            )));
            let normal_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                (b"Off".to_vec(), Object::Reference(off_id)),
                (state.clone(), Object::Reference(on_id)),
            ])));
            let appearance_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(
                vec![(b"N".to_vec(), Object::Reference(normal_id))],
            )));
            let widget_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                (b"Type".to_vec(), Object::Name(b"Annot".to_vec())),
                (b"Subtype".to_vec(), Object::Name(b"Widget".to_vec())),
                (b"Parent".to_vec(), Object::Reference(field_id)),
                (b"P".to_vec(), Object::Reference(page_id)),
                (b"AS".to_vec(), Object::Name(b"Off".to_vec())),
                (b"AP".to_vec(), Object::Reference(appearance_id)),
            ])));
            if let Some(Object::Dictionary(field)) = doc.objects.get_mut(&field_id) {
                let kids = field
                    .get(b"Kids")
                    .ok()
                    .and_then(|o| o.as_array().ok())
                    .cloned()
                    .unwrap_or_default();
                let mut kids = kids;
                kids.push(Object::Reference(widget_id));
                field.set("Kids", Object::Array(kids));
            }
            widget_ids.push(Object::Reference(widget_id));
        }
        doc.get_object_mut(page_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Annots", Object::Array(widget_ids));

        let acroform_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![(
            b"Fields".to_vec(),
            Object::Array(vec![
                Object::Reference(text_field_id),
                Object::Reference(checkbox_id),
                Object::Reference(radio_id),
            ]),
        )])));
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
                FormFieldValue {
                    name: "fullname".into(),
                    value: "Ada Lovelace".into(),
                },
                FormFieldValue {
                    name: "subscribe".into(),
                    value: "true".into(),
                },
                FormFieldValue {
                    name: "color".into(),
                    value: "green".into(),
                },
            ],
        })
        .unwrap();

        // Verify via get_form_fields roundtrip
        let fields = get_form_fields(out_str.clone()).unwrap();
        assert_eq!(
            fields.iter().find(|f| f.name == "fullname").unwrap().value,
            "Ada Lovelace"
        );
        assert_eq!(
            fields.iter().find(|f| f.name == "subscribe").unwrap().value,
            "Subscribed"
        );
        assert_eq!(
            fields.iter().find(|f| f.name == "color").unwrap().value,
            "green"
        );

        let doc = Document::load(&out).unwrap();
        let fields: Vec<ObjectId> = {
            let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
            let acro = doc
                .get_object(root)
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"AcroForm")
                .unwrap()
                .as_reference()
                .unwrap();
            doc.get_object(acro)
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"Fields")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|o| o.as_reference().unwrap())
                .collect()
        };
        for (index, expected) in [
            (1usize, b"Subscribed".as_slice()),
            (2usize, b"green".as_slice()),
        ] {
            let field = doc.get_object(fields[index]).unwrap().as_dict().unwrap();
            let selected = field.get(b"V").unwrap().as_name().unwrap();
            assert_eq!(selected, expected);
            let widgets = field.get(b"Kids").unwrap().as_array().unwrap();
            let mut selected_widgets = 0;
            for widget in widgets {
                let widget = doc
                    .get_object(widget.as_reference().unwrap())
                    .unwrap()
                    .as_dict()
                    .unwrap();
                let appearance_state = widget.get(b"AS").unwrap().as_name().unwrap();
                if appearance_state != b"Off" {
                    assert_eq!(appearance_state, expected);
                    selected_widgets += 1;
                }
            }
            assert_eq!(selected_widgets, 1);
        }

        // NeedAppearances set on the AcroForm
        let doc = Document::load(&out_str).unwrap();
        let root_ref = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let root = doc.get_object(root_ref).unwrap().as_dict().unwrap();
        let acro_ref = root.get(b"AcroForm").unwrap().as_reference().unwrap();
        let acro = doc.get_object(acro_ref).unwrap().as_dict().unwrap();
        assert!(matches!(
            acro.get(b"NeedAppearances"),
            Ok(Object::Boolean(true))
        ));
    }

    #[test]
    fn test_fill_inline_acroform_and_direct_field_with_utf16_text() {
        let dir = TempDir::new().unwrap();
        let src = create_form_test_pdf(dir.path(), "inline-form.pdf");
        let mut doc = Document::load(&src).unwrap();
        let root_id = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let acro_id = doc
            .get_object(root_id)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"AcroForm")
            .unwrap()
            .as_reference()
            .unwrap();
        let mut acro = doc.get_object(acro_id).unwrap().as_dict().unwrap().clone();
        let mut fields = acro.get(b"Fields").unwrap().as_array().unwrap().clone();
        let first_id = fields[0].as_reference().unwrap();
        fields[0] =
            Object::Dictionary(doc.get_object(first_id).unwrap().as_dict().unwrap().clone());
        acro.set("Fields", Object::Array(fields));
        doc.get_object_mut(root_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("AcroForm", Object::Dictionary(acro));
        doc.save(&src).unwrap();

        let out = dir
            .path()
            .join("inline-form-filled.pdf")
            .to_string_lossy()
            .into_owned();
        fill_form(FillFormRequest {
            input_path: src,
            output_path: out.clone(),
            values: vec![FormFieldValue {
                name: "fullname".into(),
                value: "艾达 Lovelace".into(),
            }],
        })
        .unwrap();
        let fields = get_form_fields(out).unwrap();
        assert_eq!(
            fields
                .iter()
                .find(|field| field.name == "fullname")
                .unwrap()
                .value,
            "艾达 Lovelace"
        );
    }

    #[test]
    fn test_fill_form_no_form_fails() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "noform.pdf", 1);
        let result = fill_form(FillFormRequest {
            input_path: src,
            output_path: dir.path().join("nope.pdf").to_string_lossy().to_string(),
            values: vec![FormFieldValue {
                name: "x".into(),
                value: "y".into(),
            }],
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_get_form_fields_no_form_fails() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "noform2.pdf", 1);
        assert!(get_form_fields(src).is_err());
    }

    #[test]
    fn test_replace_text() {
        let dir = TempDir::new().unwrap();
        // Base PDF with a text run at a known position
        let base = create_test_pdf(dir.path(), "rt_base.pdf", 1);
        add_text_to_page(AddTextRequest {
            input_path: base,
            output_path: dir.path().join("rt_src.pdf").to_string_lossy().to_string(),
            page: 1,
            x: 72.0,
            y: 700.0,
            text: "Hello World".into(),
            font_size: 12.0,
            color: "#000000".into(),
        })
        .unwrap();
        let src = dir.path().join("rt_src.pdf").to_string_lossy().to_string();
        let out = dir.path().join("rt_out.pdf");
        let out_str = out.to_string_lossy().to_string();

        replace_text(ReplaceTextRequest {
            input_path: src,
            output_path: out_str.clone(),
            replacements: vec![TextReplacement {
                page: 1,
                cover_x: 70.0,
                cover_y: 697.0,
                cover_width: 90.0,
                cover_height: 15.0,
                baseline_y: 700.0,
                font_size: 12.0,
                new_text: "Bye PDF".into(),
                color: None,
            }],
        })
        .unwrap();

        // The output loads and the appended content stream contains the
        // cover rect plus the replacement text
        let doc = Document::load(&out_str).unwrap();
        assert_eq!(doc.get_pages().len(), 1);
        let page_id = *doc.get_pages().get(&1).unwrap();
        let stream_ids = doc.get_page_contents(page_id);
        let mut found_cover = false;
        let mut found_text = false;
        for sid in stream_ids {
            if let Ok(stream) = doc.get_object(sid).unwrap().as_stream() {
                // decompressed_content() errors on streams without /Filter;
                // fall back to the raw content in that case
                let raw = stream
                    .get_plain_content()
                    .unwrap_or_else(|_| stream.content.clone());
                let data = String::from_utf8_lossy(&raw).to_string();
                if data.contains("re f") {
                    found_cover = true;
                }
                if data.contains("(Bye PDF) Tj") {
                    found_text = true;
                }
            }
        }
        assert!(found_cover, "cover rectangle missing");
        assert!(found_text, "replacement text missing");
    }

    #[test]
    fn test_replace_text_empty_fails() {
        let dir = TempDir::new().unwrap();
        let src = create_test_pdf(dir.path(), "rt_empty.pdf", 1);
        let result = replace_text(ReplaceTextRequest {
            input_path: src,
            output_path: dir.path().join("x.pdf").to_string_lossy().to_string(),
            replacements: vec![],
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_page_texts() {
        let dir = TempDir::new().unwrap();
        let base = create_test_pdf(dir.path(), "pt_base.pdf", 3);
        let src = dir.path().join("pt_src.pdf").to_string_lossy().to_string();
        add_text_to_page(AddTextRequest {
            input_path: base,
            output_path: src.clone(),
            page: 2,
            x: 72.0,
            y: 700.0,
            text: "Second page text".into(),
            font_size: 12.0,
            color: "#000000".into(),
        })
        .unwrap();

        let texts = extract_page_texts(src).unwrap();
        assert_eq!(texts.len(), 3);
        assert!(texts[1].contains("Second page text"));
        assert!(!texts[0].contains("Second page text"));
    }

    #[test]
    fn test_extract_page_texts_missing_file_fails() {
        assert!(extract_page_texts("/nonexistent/x.pdf".into()).is_err());
    }

    #[test]
    fn test_set_outline_structure_and_chinese_titles() {
        let dir = TempDir::new().unwrap();
        let input = create_test_pdf(dir.path(), "in.pdf", 3);
        let out = dir.path().join("out.pdf");
        set_outline(SetOutlineRequest {
            input_path: input,
            output_path: out.to_string_lossy().to_string(),
            items: vec![
                OutlineItemInput {
                    title: "第一章 概述".into(),
                    page: 1,
                    children: vec![
                        OutlineItemInput {
                            title: "1.1 背景".into(),
                            page: 2,
                            children: vec![],
                        },
                        OutlineItemInput {
                            title: "1.2 目标".into(),
                            page: 3,
                            children: vec![],
                        },
                    ],
                },
                OutlineItemInput {
                    title: "Chapter 2".into(),
                    page: 3,
                    children: vec![],
                },
            ],
        })
        .unwrap();

        let doc = Document::load(&out).unwrap();
        let root_ref = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let outlines_ref = doc
            .get_object(root_ref)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Outlines")
            .unwrap()
            .as_reference()
            .unwrap();
        let outlines = doc.get_object(outlines_ref).unwrap().as_dict().unwrap();
        assert_eq!(outlines.get(b"Count").unwrap(), &Object::Integer(4));

        // First top-level item: Chinese title stored as UTF-16BE with BOM
        let first_ref = outlines.get(b"First").unwrap().as_reference().unwrap();
        let first = doc.get_object(first_ref).unwrap().as_dict().unwrap();
        assert_eq!(
            first.get(b"Title").unwrap(),
            &Object::String(encode_pdf_text("第一章 概述"), lopdf::StringFormat::Literal)
        );

        // /Dest points at the page-1 object with /XYZ nulls
        let page1 = *doc.get_pages().get(&1).unwrap();
        match first.get(b"Dest").unwrap() {
            Object::Array(arr) => {
                assert_eq!(arr[0], Object::Reference(page1));
                assert_eq!(arr[1], Object::Name(b"XYZ".to_vec()));
                assert_eq!(arr[2], Object::Null);
            }
            o => panic!("dest not array: {:?}", o),
        }

        // Children linked: /First //Last /Count=2, child /Parent back-link, sibling /Next
        assert_eq!(first.get(b"Count").unwrap(), &Object::Integer(2));
        let child1_ref = first.get(b"First").unwrap().as_reference().unwrap();
        let child1 = doc.get_object(child1_ref).unwrap().as_dict().unwrap();
        assert_eq!(
            child1.get(b"Parent").unwrap(),
            &Object::Reference(first_ref)
        );
        assert_eq!(
            child1.get(b"Next").unwrap().as_reference().unwrap(),
            first.get(b"Last").unwrap().as_reference().unwrap()
        );

        // Last top-level item: /Prev back to first, no /Next
        let last_ref = outlines.get(b"Last").unwrap().as_reference().unwrap();
        let last = doc.get_object(last_ref).unwrap().as_dict().unwrap();
        assert_eq!(
            last.get(b"Prev").unwrap().as_reference().unwrap(),
            first_ref
        );
        assert!(last.get(b"Next").is_err());
    }

    #[test]
    fn test_set_outline_empty_clears() {
        let dir = TempDir::new().unwrap();
        let input = create_test_pdf(dir.path(), "in.pdf", 2);
        let out = dir.path().join("out.pdf");
        // First set a non-empty outline, then replace with an empty one
        set_outline(SetOutlineRequest {
            input_path: input.clone(),
            output_path: out.to_string_lossy().to_string(),
            items: vec![OutlineItemInput {
                title: "A".into(),
                page: 1,
                children: vec![],
            }],
        })
        .unwrap();
        set_outline(SetOutlineRequest {
            input_path: out.to_string_lossy().to_string(),
            output_path: out.to_string_lossy().to_string(),
            items: vec![],
        })
        .unwrap();

        let doc = Document::load(&out).unwrap();
        let root_ref = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let outlines_ref = doc
            .get_object(root_ref)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Outlines")
            .unwrap()
            .as_reference()
            .unwrap();
        let outlines = doc.get_object(outlines_ref).unwrap().as_dict().unwrap();
        assert_eq!(outlines.get(b"Count").unwrap(), &Object::Integer(0));
        assert!(outlines.get(b"First").is_err());
    }

    #[test]
    fn test_set_outline_invalid_page_fails() {
        let dir = TempDir::new().unwrap();
        let input = create_test_pdf(dir.path(), "in.pdf", 2);
        let out = dir.path().join("out.pdf");
        let result = set_outline(SetOutlineRequest {
            input_path: input,
            output_path: out.to_string_lossy().to_string(),
            items: vec![OutlineItemInput {
                title: "X".into(),
                page: 99,
                children: vec![],
            }],
        });
        assert!(result.is_err());
    }
}
