use crate::commands::validation::validate_path;
use crate::config::OcrConfig;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

// ─── Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrTextBox {
    pub points: Vec<[f32; 2]>,
    pub text: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrLanguage {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrEngineStatus {
    pub initialized: bool,
    pub backend: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrStructuredBlock {
    pub points: Vec<[f32; 2]>,
    pub text: String,
    pub confidence: f32,
    pub element_type: String,
    pub reading_order: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfAnalysis {
    pub total_pages: u32,
    pub text_pages: u32,
    pub image_pages: u32,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrModelSet {
    pub det_path: String,
    pub rec_path: String,
    pub keys_path: String,
    pub language: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrSuggestedModel {
    pub name: String,
    pub language: String,
    pub description: String,
    pub det_url: String,
    pub rec_url: String,
    pub keys_url: String,
    pub total_size: String,
}

const MODEL_BASE_URL: &str =
    "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/next/models";

// ─── Engine State ─────────────────────────────────────────────────────

struct OcrEngineHolder {
    engine: ocr_rs::OcrEngine,
    language: String,
    backend: String,
    model_dir: String,
    det_model: String,
    rec_model: String,
    keys_file: String,
}

static OCR_ENGINE: Mutex<Option<OcrEngineHolder>> = Mutex::new(None);

// ─── Config helpers ───────────────────────────────────────────────────

fn get_ocr_config(app_handle: &tauri::AppHandle) -> AppResult<Option<OcrConfig>> {
    let state = app_handle.state::<Mutex<crate::config::AppConfig>>();
    let config = state
        .lock()
        .map_err(|e| AppError::Ocr(format!("Lock: {}", e)))?;
    Ok(config.ocr.clone())
}

fn set_ocr_config(app_handle: &tauri::AppHandle, ocr: Option<OcrConfig>) -> AppResult<()> {
    let state = app_handle.state::<Mutex<crate::config::AppConfig>>();
    let mut config = state
        .lock()
        .map_err(|e| AppError::Ocr(format!("Lock: {}", e)))?;
    config.ocr = ocr;
    crate::config::save_config_with_handle(app_handle, &config)?;
    Ok(())
}

// ─── Model Paths ──────────────────────────────────────────────────────

fn default_models_dir(app_handle: &tauri::AppHandle) -> AppResult<PathBuf> {
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .map_err(|e| AppError::Ocr(format!("Resource dir: {}", e)))?;
    Ok(resource_dir.join("models").join("ocr"))
}

fn resolve_model_paths(
    app_handle: &tauri::AppHandle,
    language: &str,
) -> AppResult<(PathBuf, PathBuf, PathBuf)> {
    // Prefer OcrConfig if set
    if let Some(ref ocr_cfg) = get_ocr_config(app_handle)? {
        let dir = PathBuf::from(&ocr_cfg.model_dir);

        // Detection model: use configured name or default
        let det = dir.join(&ocr_cfg.det_model);
        let keys = dir.join(&ocr_cfg.keys_file);

        // Recognition model: try language-specific first, then configured default
        let rec = if language != "default" && !language.is_empty() {
            let lang_rec = find_lang_rec_model(&dir, language);
            lang_rec.unwrap_or_else(|| dir.join(&ocr_cfg.rec_model))
        } else {
            dir.join(&ocr_cfg.rec_model)
        };

        return Ok((det, rec, keys));
    }

    // Fallback to bundled models
    let models = default_models_dir(app_handle)?;
    let det = models.join("PP-OCRv5_mobile_det.mnn");
    let keys = models.join("ppocr_keys_v5.txt");

    let rec = if language != "default" && !language.is_empty() {
        let lang_rec = find_lang_rec_model(&models, language);
        lang_rec.unwrap_or_else(|| models.join("PP-OCRv5_mobile_rec.mnn"))
    } else {
        models.join("PP-OCRv5_mobile_rec.mnn")
    };

    Ok((det, rec, keys))
}

fn find_lang_rec_model(dir: &PathBuf, language: &str) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&format!("{}_PP-OCRv5_mobile_rec", language))
                && name.ends_with(".mnn")
            {
                return Some(entry.path());
            }
        }
    }
    None
}

// ─── Engine Init ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn ocr_init_engine(
    app_handle: tauri::AppHandle,
    language: Option<String>,
    gpu_enabled: Option<bool>,
) -> AppResult<OcrEngineStatus> {
    let lang = language.unwrap_or_else(|| {
        get_ocr_config(&app_handle)
            .ok()
            .and_then(|c| c.map(|c| c.language))
            .unwrap_or_else(|| "default".to_string())
    });

    let use_gpu = gpu_enabled.unwrap_or_else(|| {
        get_ocr_config(&app_handle)
            .ok()
            .and_then(|c| c.map(|c| c.gpu_enabled))
            .unwrap_or(true)
    });

    let (det, rec, keys) = resolve_model_paths(&app_handle, &lang)?;

    for (name, path) in [("detection", &det), ("recognition", &rec), ("keys", &keys)] {
        if !path.exists() {
            return Err(AppError::Ocr(format!(
                "OCR model '{}' not found at {:?}",
                name, path
            )));
        }
    }

    let backend_name;
    let config = if use_gpu {
        #[cfg(target_os = "macos")]
        {
            backend_name = "metal";
            Some(
                ocr_rs::OcrEngineConfig::new()
                    .with_backend(ocr_rs::Backend::Metal),
            )
        }
        #[cfg(not(target_os = "macos"))]
        {
            backend_name = "cpu";
            None
        }
    } else {
        backend_name = "cpu";
        None
    };

    let engine = ocr_rs::OcrEngine::new(
        det.to_str().unwrap(),
        rec.to_str().unwrap(),
        keys.to_str().unwrap(),
        config,
    )
    .map_err(|e| AppError::Ocr(format!("Engine init failed: {}", e)))?;

    // Record effective model paths for language switching
    let model_dir = det.parent().unwrap_or(Path::new(".")).to_string_lossy().to_string();
    let det_model = det.file_name().unwrap_or_default().to_string_lossy().to_string();
    let rec_model = rec.file_name().unwrap_or_default().to_string_lossy().to_string();
    let keys_file = keys.file_name().unwrap_or_default().to_string_lossy().to_string();

    let mut guard = OCR_ENGINE
        .lock()
        .map_err(|e| AppError::Ocr(format!("Lock: {}", e)))?;

    *guard = Some(OcrEngineHolder {
        engine,
        language: lang.clone(),
        backend: backend_name.to_string(),
        model_dir,
        det_model,
        rec_model,
        keys_file,
    });

    Ok(OcrEngineStatus {
        initialized: true,
        backend: backend_name.to_string(),
        language: lang,
    })
}

// ─── Internal: run recognition (sync, caller must hold no lock) ──────

fn recognize_image(image_path: &str) -> AppResult<Vec<OcrTextBox>> {
    let guard = OCR_ENGINE
        .lock()
        .map_err(|e| AppError::Ocr(format!("Lock: {}", e)))?;
    let holder = guard
        .as_ref()
        .ok_or_else(|| AppError::Ocr("OCR engine not initialized".into()))?;

    if !std::path::Path::new(image_path).exists() {
        return Err(AppError::Ocr(format!(
            "Image not found: {}",
            image_path
        )));
    }

    let img = image::open(image_path)
        .map_err(|e| AppError::Ocr(format!("Failed to load image: {}", e)))?;

    let results = holder
        .engine
        .recognize(&img)
        .map_err(|e| AppError::Ocr(format!("Recognition failed: {}", e)))?;

    let boxes: Vec<OcrTextBox> = results
        .iter()
        .map(|r| {
            let points = if let Some(pts) = &r.bbox.points {
                vec![
                    [pts[0].x as f32, pts[0].y as f32],
                    [pts[1].x as f32, pts[1].y as f32],
                    [pts[2].x as f32, pts[2].y as f32],
                    [pts[3].x as f32, pts[3].y as f32],
                ]
            } else {
                let rect = &r.bbox.rect;
                vec![
                    [rect.left() as f32, rect.top() as f32],
                    [rect.right() as f32, rect.top() as f32],
                    [rect.right() as f32, rect.bottom() as f32],
                    [rect.left() as f32, rect.bottom() as f32],
                ]
            };
            OcrTextBox {
                points,
                text: r.text.clone(),
                confidence: r.confidence,
            }
        })
        .collect();

    Ok(boxes)
}

// ─── Tauri Command: Recognize ─────────────────────────────────────────

#[tauri::command]
pub async fn ocr_recognize(
    app_handle: tauri::AppHandle,
    image_path: String,
    language: Option<String>,
) -> AppResult<Vec<OcrTextBox>> {
    validate_path(&image_path)?;

    // Auto-init if engine not ready
    let needs_init = {
        let guard = OCR_ENGINE
            .lock()
            .map_err(|e| AppError::Ocr(format!("Lock: {}", e)))?;
        guard.is_none()
    };
    if needs_init {
        ocr_init_engine(app_handle.clone(), language.clone(), Some(true)).await?;
    }

    // Re-init if language changed
    if let Some(ref lang) = language {
        let needs_reinit = {
            let guard = OCR_ENGINE
                .lock()
                .map_err(|e| AppError::Ocr(format!("Lock: {}", e)))?;
            guard
                .as_ref()
                .map_or(true, |h| h.language != *lang)
        };
        if needs_reinit {
            ocr_init_engine(app_handle, Some(lang.clone()), Some(true)).await?;
        }
    }

    recognize_image(&image_path)
}

// ─── Tauri Command: Structured Recognition ───────────────────────────

#[tauri::command]
pub async fn ocr_recognize_structured(
    app_handle: tauri::AppHandle,
    image_path: String,
    language: Option<String>,
    page_height: f32,
) -> AppResult<Vec<OcrStructuredBlock>> {
    validate_path(&image_path)?;

    let needs_init = {
        let guard = OCR_ENGINE
            .lock()
            .map_err(|e| AppError::Ocr(format!("Lock: {}", e)))?;
        guard.is_none()
    };
    if needs_init {
        ocr_init_engine(app_handle.clone(), language.clone(), Some(true)).await?;
    }

    let raw_boxes = recognize_image(&image_path)?;

    // Sort by Y (top to bottom), then X (left to right)
    let mut sorted: Vec<_> = raw_boxes.into_iter().collect();
    sorted.sort_by(|a, b| {
        let ay = a.points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let by = b.points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        ay.partial_cmp(&by)
            .unwrap()
            .then_with(|| {
                let ax = a.points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
                let bx = b.points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
                ax.partial_cmp(&bx).unwrap()
            })
    });

    // Compute median block height for relative sizing
    let heights: Vec<f32> = sorted
        .iter()
        .map(|b| {
            let ys: Vec<f32> = b.points.iter().map(|p| p[1]).collect();
            ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
                - ys.iter().cloned().fold(f32::INFINITY, f32::min)
        })
        .filter(|&h| h > 0.0)
        .collect();
    let median_height = if heights.is_empty() {
        12.0
    } else {
        let mut h = heights;
        h.sort_by(|a, b| a.partial_cmp(b).unwrap());
        h[h.len() / 2]
    };

    let structured: Vec<OcrStructuredBlock> = sorted
        .into_iter()
        .enumerate()
        .map(|(order, b)| {
            let block_h = {
                let ys: Vec<f32> = b.points.iter().map(|p| p[1]).collect();
                ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
                    - ys.iter().cloned().fold(f32::INFINITY, f32::min)
            };
            let min_y = b.points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
            let rel_size = if median_height > 0.0 {
                block_h / median_height
            } else {
                1.0
            };
            let rel_y = if page_height > 0.0 {
                min_y / page_height
            } else {
                0.5
            };

            let element_type = if rel_y < 0.08 {
                "header"
            } else if rel_y > 0.92 {
                "footer"
            } else if rel_size > 1.6 {
                "title"
            } else if rel_size > 1.25 {
                "heading"
            } else {
                "text"
            };

            OcrStructuredBlock {
                points: b.points,
                text: b.text,
                confidence: b.confidence,
                element_type: element_type.to_string(),
                reading_order: order,
            }
        })
        .collect();

    Ok(structured)
}

// ─── Language Management ─────────────────────────────────────────────

#[tauri::command]
pub async fn ocr_get_languages(
    app_handle: tauri::AppHandle,
) -> AppResult<Vec<OcrLanguage>> {
    let models_dir = if let Some(ref cfg) = get_ocr_config(&app_handle)? {
        PathBuf::from(&cfg.model_dir)
    } else {
        default_models_dir(&app_handle)?
    };

    let mut languages = Vec::new();

    // Check default (non-prefixed) rec model
    let default_rec = models_dir.join("PP-OCRv5_mobile_rec.mnn");
    let default_keys = models_dir.join("ppocr_keys_v5.txt");
    if default_rec.exists() && default_keys.exists() {
        languages.push(OcrLanguage {
            code: "default".to_string(),
            name: "Chinese / English / Japanese".to_string(),
        });
    }

    // Scan for language-prefixed rec models
    if let Ok(entries) = std::fs::read_dir(&models_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.contains("_PP-OCRv5_mobile_rec") && name.ends_with(".mnn") {
                let code = name.split("_PP-OCRv5_mobile_rec").next().unwrap_or("");
                if code.is_empty() {
                    continue;
                }
                // Check for matching keys file
                let keys_name = format!("ppocr_keys_{}.txt", code);
                let has_keys = models_dir.join(&keys_name).exists()
                    || default_keys.exists();
                if has_keys {
                    languages.push(OcrLanguage {
                        code: code.to_string(),
                        name: language_display_name(code),
                    });
                }
            }
        }
    }

    Ok(languages)
}

fn language_display_name(code: &str) -> String {
    match code {
        "korean" => "Korean".to_string(),
        "latin" => "Latin (French, German, Spanish...)".to_string(),
        "eslav" => "East Slavic (Russian, Ukrainian...)".to_string(),
        "arabic" => "Arabic / Persian".to_string(),
        "cyrillic" => "Cyrillic".to_string(),
        "devanagari" => "Devanagari (Hindi...)".to_string(),
        "th" => "Thai".to_string(),
        "el" => "Greek".to_string(),
        "en" => "English".to_string(),
        "ta" => "Tamil".to_string(),
        "te" => "Telugu".to_string(),
        _ => code.to_string(),
    }
}

// ─── Engine Status ────────────────────────────────────────────────────

#[tauri::command]
pub async fn ocr_get_status() -> AppResult<OcrEngineStatus> {
    let guard = OCR_ENGINE
        .lock()
        .map_err(|e| AppError::Ocr(format!("Lock: {}", e)))?;
    match guard.as_ref() {
        Some(h) => Ok(OcrEngineStatus {
            initialized: true,
            backend: h.backend.clone(),
            language: h.language.clone(),
        }),
        None => Ok(OcrEngineStatus {
            initialized: false,
            backend: String::new(),
            language: String::new(),
        }),
    }
}

// ─── PDF Analysis (Smart Triage) ─────────────────────────────────────

#[tauri::command]
pub async fn ocr_analyze_pdf(file_path: String) -> AppResult<PdfAnalysis> {
    validate_path(&file_path)?;

    let doc = lopdf::Document::load(&file_path)
        .map_err(|e| AppError::Ocr(format!("Failed to load PDF: {}", e)))?;
    let total_pages = doc.get_pages().len() as u32;

    let mut text_pages = 0u32;

    for (&page_num, _) in doc.get_pages().iter() {
        if let Ok(text) = doc.extract_text(&[page_num]) {
            let text_ops = text.matches("Tj").count()
                + text.matches("TJ").count()
                + text.matches(" T*").count();
            if text_ops > 3 {
                text_pages += 1;
            }
        }
    }

    let image_pages = total_pages.saturating_sub(text_pages);

    let recommendation = if image_pages == 0 {
        "text"
    } else if text_pages == 0 {
        "ocr"
    } else if image_pages > text_pages {
        "ocr"
    } else {
        "mixed"
    };

    Ok(PdfAnalysis {
        total_pages,
        text_pages,
        image_pages,
        recommendation: recommendation.to_string(),
    })
}

// ─── Model Scanning ──────────────────────────────────────────────────

#[tauri::command]
pub async fn ocr_scan_models(dir_path: String) -> AppResult<Vec<OcrModelSet>> {
    let dir = PathBuf::from(&dir_path);
    if !dir.is_dir() {
        return Err(AppError::Ocr(format!("Not a directory: {}", dir_path)));
    }

    let mut det_files = Vec::new();
    let mut rec_files = Vec::new();
    let mut keys_files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if name.contains("det") && name.ends_with(".mnn") {
                det_files.push((name.clone(), path));
            } else if name.contains("rec") && name.ends_with(".mnn") {
                rec_files.push((name.clone(), path));
            } else if name.contains("keys") && name.ends_with(".txt") {
                keys_files.push((name.clone(), path));
            }
        }
    }

    let mut model_sets = Vec::new();

    // Match default model set
    let default_det = det_files.iter().find(|(n, _)| n.contains("PP-OCRv5_mobile_det"));
    let default_rec = rec_files
        .iter()
        .find(|(n, _)| n == "PP-OCRv5_mobile_rec.mnn" || (!n.contains('_') && n.contains("PP-OCRv5")));
    let default_keys = keys_files
        .iter()
        .find(|(n, _)| n == "ppocr_keys_v5.txt" || n == "ppocr_keys.txt");

    if let (Some(det), Some(rec), Some(keys)) = (default_det, default_rec, default_keys) {
        model_sets.push(OcrModelSet {
            det_path: det.1.to_string_lossy().to_string(),
            rec_path: rec.1.to_string_lossy().to_string(),
            keys_path: keys.1.to_string_lossy().to_string(),
            language: "default".to_string(),
            display_name: "Chinese / English / Japanese (PP-OCRv5)".to_string(),
        });
    }

    // Match language-specific model sets
    for (rec_name, rec_path) in &rec_files {
        let code = rec_name.split("_PP-OCRv5_mobile_rec").next().unwrap_or("");
        if code.is_empty() {
            continue;
        }
        let lang_keys = keys_files.iter().find(|(n, _)| {
            n.contains(code) || n == "ppocr_keys_v5.txt" || n == "ppocr_keys.txt"
        });

        let det_match = det_files.iter().find(|(n, _)| n.contains("PP-OCRv5_mobile_det"));

        if let (Some(det), Some(keys)) = (det_match, lang_keys) {
            model_sets.push(OcrModelSet {
                det_path: det.1.to_string_lossy().to_string(),
                rec_path: rec_path.to_string_lossy().to_string(),
                keys_path: keys.1.to_string_lossy().to_string(),
                language: code.to_string(),
                display_name: format!("{} ({})", language_display_name(code), code),
            });
        }
    }

    Ok(model_sets)
}

// ─── Model Validation ────────────────────────────────────────────────

#[tauri::command]
pub async fn ocr_validate_models(
    det_path: String,
    rec_path: String,
    keys_path: String,
) -> AppResult<bool> {
    for (name, path) in [
        ("detection", &det_path),
        ("recognition", &rec_path),
        ("keys", &keys_path),
    ] {
        if !std::path::Path::new(path).exists() {
            return Err(AppError::Ocr(format!(
                "OCR model '{}' not found at {}",
                name, path
            )));
        }
    }

    let config = ocr_rs::OcrEngineConfig::new();

    match ocr_rs::OcrEngine::new(&det_path, &rec_path, &keys_path, Some(config)) {
        Ok(_) => Ok(true),
        Err(e) => Err(AppError::Ocr(format!("Model validation failed: {}", e))),
    }
}

// ─── Set Model Directory ─────────────────────────────────────────────

#[tauri::command]
pub async fn ocr_set_model_dir(
    app_handle: tauri::AppHandle,
    dir_path: String,
    det_model: String,
    rec_model: String,
    keys_file: String,
    language: String,
    gpu_enabled: bool,
) -> AppResult<OcrEngineStatus> {
    let dir = PathBuf::from(&dir_path);
    if !dir.is_dir() {
        return Err(AppError::Ocr(format!("Directory not found: {}", dir_path)));
    }

    // Verify files exist
    for (name, file) in [
        ("det", &det_model),
        ("rec", &rec_model),
        ("keys", &keys_file),
    ] {
        if !dir.join(file).exists() {
            return Err(AppError::Ocr(format!(
                "Model file '{}' not found: {:?}",
                name,
                dir.join(file)
            )));
        }
    }

    let ocr_config = OcrConfig {
        model_dir: dir_path,
        det_model,
        rec_model,
        keys_file,
        language: language.clone(),
        gpu_enabled,
    };

    set_ocr_config(&app_handle, Some(ocr_config))?;

    // Reset engine so it re-initializes with new paths
    {
        let mut guard = OCR_ENGINE
            .lock()
            .map_err(|e| AppError::Ocr(format!("Lock: {}", e)))?;
        *guard = None;
    }

    // Initialize with new config
    ocr_init_engine(app_handle, Some(language), Some(gpu_enabled)).await
}

// ─── Suggested Models ────────────────────────────────────────────────

#[tauri::command]
pub async fn ocr_get_suggested_models(
    _app_handle: tauri::AppHandle,
) -> AppResult<Vec<OcrSuggestedModel>> {
    let base = MODEL_BASE_URL;
    Ok(vec![
        OcrSuggestedModel {
            name: "PP-OCRv5 Mobile".to_string(),
            language: "default".to_string(),
            description: "Chinese / English / Japanese".to_string(),
            det_url: format!("{}/PP-OCRv5_mobile_det.mnn", base),
            rec_url: format!("{}/PP-OCRv5_mobile_rec.mnn", base),
            keys_url: format!("{}/ppocr_keys_v5.txt", base),
            total_size: "~21 MB".to_string(),
        },
        OcrSuggestedModel {
            name: "PP-OCRv5 Korean".to_string(),
            language: "korean".to_string(),
            description: "Korean (requires shared det model)".to_string(),
            det_url: String::new(),
            rec_url: format!("{}/korean_PP-OCRv5_mobile_rec_infer.mnn", base),
            keys_url: format!("{}/ppocr_keys_korean.txt", base),
            total_size: "~10 MB".to_string(),
        },
        OcrSuggestedModel {
            name: "PP-OCRv5 Latin".to_string(),
            language: "latin".to_string(),
            description: "French, German, Spanish, Italian... (requires shared det model)".to_string(),
            det_url: String::new(),
            rec_url: format!("{}/latin_PP-OCRv5_mobile_rec_infer.mnn", base),
            keys_url: format!("{}/ppocr_keys_latin.txt", base),
            total_size: "~10 MB".to_string(),
        },
        OcrSuggestedModel {
            name: "PP-OCRv5 Arabic".to_string(),
            language: "arabic".to_string(),
            description: "Arabic / Persian (requires shared det model)".to_string(),
            det_url: String::new(),
            rec_url: format!("{}/arabic_PP-OCRv5_mobile_rec_infer.mnn", base),
            keys_url: format!("{}/ppocr_keys_arabic.txt", base),
            total_size: "~10 MB".to_string(),
        },
        OcrSuggestedModel {
            name: "PP-OCRv5 Cyrillic".to_string(),
            language: "cyrillic".to_string(),
            description: "Russian, Ukrainian... (requires shared det model)".to_string(),
            det_url: String::new(),
            rec_url: format!("{}/cyrillic_PP-OCRv5_mobile_rec_infer.mnn", base),
            keys_url: format!("{}/ppocr_keys_cyrillic.txt", base),
            total_size: "~10 MB".to_string(),
        },
    ])
}

// ─── Download Model ──────────────────────────────────────────────────

#[tauri::command]
pub async fn ocr_download_model(
    _app_handle: tauri::AppHandle,
    url: String,
    save_path: String,
    proxy: Option<String>,
) -> AppResult<()> {
    if url.is_empty() || !url.starts_with("https://") {
        return Err(AppError::Ocr("URL must use HTTPS scheme".to_string()));
    }

    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300));

    if let Some(ref proxy_url) = proxy {
        if !proxy_url.is_empty() {
            let proxy = reqwest::Proxy::all(proxy_url)
                .map_err(|e| AppError::Ocr(format!("Invalid proxy: {}", e)))?;
            builder = builder.proxy(proxy);
        }
    }

    let client = builder
        .build()
        .map_err(|e| AppError::Ocr(format!("HTTP client error: {}", e)))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::Ocr(format!("Download failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::Ocr(format!(
            "Download failed with status: {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::Ocr(format!("Failed to read response: {}", e)))?;

    let path = PathBuf::from(&save_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&path, &bytes)?;
    Ok(())
}

// ─── Download PaddleOCR Default Bundle (one-click) ───────────────────

#[tauri::command]
pub async fn ocr_download_paddle_bundle(
    app_handle: tauri::AppHandle,
    proxy: Option<String>,
) -> AppResult<OcrEngineStatus> {
    let models_dir = default_models_dir(&app_handle)?;
    std::fs::create_dir_all(&models_dir)?;

    let base = MODEL_BASE_URL;
    let files = [
        ("PP-OCRv5_mobile_det.mnn", format!("{}/PP-OCRv5_mobile_det.mnn", base)),
        ("PP-OCRv5_mobile_rec.mnn", format!("{}/PP-OCRv5_mobile_rec.mnn", base)),
        ("ppocr_keys_v5.txt", format!("{}/ppocr_keys_v5.txt", base)),
    ];

    for (filename, url) in &files {
        let save_path = models_dir.join(filename);
        ocr_download_model(app_handle.clone(), url.clone(), save_path.to_string_lossy().to_string(), proxy.clone()).await?;
    }

    // Auto-configure and init engine
    let det = models_dir.join("PP-OCRv5_mobile_det.mnn");
    let rec = models_dir.join("PP-OCRv5_mobile_rec.mnn");
    let keys = models_dir.join("ppocr_keys_v5.txt");

    let ocr_config = OcrConfig {
        model_dir: models_dir.to_string_lossy().to_string(),
        det_model: "PP-OCRv5_mobile_det.mnn".to_string(),
        rec_model: "PP-OCRv5_mobile_rec.mnn".to_string(),
        keys_file: "ppocr_keys_v5.txt".to_string(),
        language: "default".to_string(),
        gpu_enabled: true,
    };
    set_ocr_config(&app_handle, Some(ocr_config))?;

    ocr_init_engine(app_handle, Some("default".to_string()), Some(true)).await
}

// ─── Check if OCR is configured ──────────────────────────────────────

#[tauri::command]
pub async fn ocr_check_configured(app_handle: tauri::AppHandle) -> AppResult<bool> {
    let config = get_ocr_config(&app_handle)?;
    match config {
        Some(ref cfg) => {
            let dir = PathBuf::from(&cfg.model_dir);
            Ok(dir.join(&cfg.det_model).exists()
                && dir.join(&cfg.rec_model).exists()
                && dir.join(&cfg.keys_file).exists())
        }
        None => {
            let models = default_models_dir(&app_handle)?;
            Ok(models.join("PP-OCRv5_mobile_det.mnn").exists()
                && models.join("PP-OCRv5_mobile_rec.mnn").exists()
                && models.join("ppocr_keys_v5.txt").exists())
        }
    }
}

// ─── Get app model directory path (for custom model setup) ───────────

#[tauri::command]
pub async fn ocr_get_model_dir(app_handle: tauri::AppHandle) -> AppResult<String> {
    let dir = default_models_dir(&app_handle)?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir.to_string_lossy().to_string())
}

// ─── Rescan and apply models from app model directory ────────────────

#[tauri::command]
pub async fn ocr_apply_local_models(
    app_handle: tauri::AppHandle,
    language: String,
    gpu_enabled: bool,
) -> AppResult<OcrEngineStatus> {
    let models_dir = default_models_dir(&app_handle)?;

    // Determine file names based on language
    let (det_name, rec_name, keys_name): (String, String, String) = if language == "default" {
        (
            "PP-OCRv5_mobile_det.mnn".to_string(),
            "PP-OCRv5_mobile_rec.mnn".to_string(),
            "ppocr_keys_v5.txt".to_string(),
        )
    } else {
        (
            "PP-OCRv5_mobile_det.mnn".to_string(),
            format!("{}_PP-OCRv5_mobile_rec_infer.mnn", language),
            format!("ppocr_keys_{}.txt", language),
        )
    };

    for (name, path) in [("det", models_dir.join(&det_name)), ("rec", models_dir.join(&rec_name)), ("keys", models_dir.join(&keys_name))] {
        if !path.exists() {
            return Err(AppError::Ocr(format!(
                "Model file '{}' not found at {:?}. Please copy the file to the model directory.",
                name, path
            )));
        }
    }

    let ocr_config = OcrConfig {
        model_dir: models_dir.to_string_lossy().to_string(),
        det_model: det_name.clone(),
        rec_model: rec_name.clone(),
        keys_file: keys_name.clone(),
        language: language.clone(),
        gpu_enabled,
    };

    set_ocr_config(&app_handle, Some(ocr_config))?;

    // Reset engine
    {
        let mut guard = OCR_ENGINE
            .lock()
            .map_err(|e| AppError::Ocr(format!("Lock: {}", e)))?;
        *guard = None;
    }

    ocr_init_engine(app_handle, Some(language), Some(gpu_enabled)).await
}
