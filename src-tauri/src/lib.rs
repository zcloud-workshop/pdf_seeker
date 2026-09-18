pub mod commands;
pub mod config;
pub mod error;
pub mod pdf;

use std::sync::Mutex;

pub fn run() {
    let initial_config = config::AppConfig::default();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(Mutex::new(initial_config))
        .manage(commands::recent::RecentFilesState(Mutex::new(Vec::new())))
        .setup(|app| {
            config::init(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::config::get_config,
            commands::config::update_config,
            commands::recent::get_recent_files,
            commands::recent::add_recent_file,
            commands::recent::clear_recent_files,
            commands::pdf_ops::merge_pdfs,
            commands::pdf_ops::rotate_pdf,
            commands::pdf_ops::delete_pages,
            commands::pdf_ops::extract_text,
            commands::pdf_ops::split_pdf,
            commands::pdf_ops::extract_pages_pdf,
            commands::pdf_ops::compress_pdf,
            commands::pdf_ops::add_text_watermark,
            commands::pdf_ops::images_to_pdf,
            commands::pdf_ops::reorder_pages,
            commands::pdf_ops::insert_pages,
            commands::pdf_ops::sign_pdf,
            commands::ocr::ocr_init_engine,
            commands::ocr::ocr_recognize,
            commands::ocr::ocr_recognize_structured,
            commands::ocr::ocr_get_languages,
            commands::ocr::ocr_get_status,
            commands::ocr::ocr_analyze_pdf,
            commands::ocr::ocr_scan_models,
            commands::ocr::ocr_validate_models,
            commands::ocr::ocr_set_model_dir,
            commands::ocr::ocr_get_suggested_models,
            commands::ocr::ocr_download_model,
            commands::ocr::ocr_download_paddle_bundle,
            commands::ocr::ocr_check_configured,
            commands::ocr::ocr_get_model_dir,
            commands::ocr::ocr_apply_local_models,
            commands::pdf_ops::get_temp_dir,
            commands::pdf_ops::save_image_file,
            commands::pdf_ops::add_text_to_page,
            commands::pdf_ops::add_rectangle,
            commands::pdf_ops::add_highlight,
            commands::pdf_ops::add_whiteout,
            commands::pdf_ops::apply_edit_operations,
            commands::pdf_ops::get_pdf_info,
            commands::pdf_ops::add_page_numbers,
            commands::pdf_ops::sanitize_pdf,
            commands::s3_ops::s3_test_connection,
            commands::s3_ops::s3_list_files,
            commands::s3_ops::s3_upload_file,
            commands::s3_ops::s3_download_file,
            commands::s3_ops::s3_delete_file,
            commands::s3_ops::s3_list_versions,
            commands::s3_ops::s3_delete_version,
            commands::s3_ops::s3_create_folder,
            commands::s3_ops::s3_get_presigned_url,
            commands::searchable_pdf::create_searchable_pdf,
            commands::searchable_pdf::postprocess_text,
            commands::ocr::ocr_cluster_paragraphs,
            commands::ocr::ocr_detect_tables,
            commands::pdf_ops::download_pdf_from_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
