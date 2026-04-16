mod commands;
mod config;
mod error;

use std::sync::Mutex;

pub fn run() {
    let initial_config = config::AppConfig::default();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(Mutex::new(initial_config))
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
            commands::pdf_ops::check_tesseract_available,
            commands::pdf_ops::ocr_extract_from_images,
            commands::pdf_ops::get_temp_dir,
            commands::pdf_ops::save_image_file,
            commands::pdf_ops::add_text_to_page,
            commands::pdf_ops::add_rectangle,
            commands::pdf_ops::add_highlight,
            commands::s3_ops::s3_test_connection,
            commands::s3_ops::s3_list_files,
            commands::s3_ops::s3_upload_file,
            commands::s3_ops::s3_download_file,
            commands::s3_ops::s3_delete_file,
            commands::s3_ops::s3_list_versions,
            commands::s3_ops::s3_delete_version,
            commands::s3_ops::s3_create_folder,
            commands::s3_ops::s3_get_presigned_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
