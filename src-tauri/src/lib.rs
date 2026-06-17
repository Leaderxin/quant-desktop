pub mod domain;
pub mod db;
pub mod datasource;
pub mod cache;
pub mod commands;

use std::sync::Arc;
use tauri::Manager;
use db::Database;
use datasource::DataSourceManager;
use cache::QuoteCache;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialize database
            let app_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            let db = Arc::new(Database::open(app_dir).expect("Failed to open database"));

            // Initialize data source manager
            let mut ds_manager = DataSourceManager::new();
            ds_manager.register(Box::new(
                crate::datasource::eastmoney::EastmoneyAdapter::new(),
            ));

            // Restore last used data source from settings
            if let Ok(Some(active)) = db.get_setting("active_datasource") {
                let _ = ds_manager.set_active(&active);
            }

            let ds_manager = Arc::new(ds_manager);

            // Initialize cache and restore from SQLite
            let cache = Arc::new(QuoteCache::new(db.clone()));
            cache.restore_from_db();

            // Manage state
            app.manage(db.clone());
            app.manage(ds_manager.clone());
            app.manage(cache.clone());

            // Start background polling
            let interval: u64 = db
                .get_setting("refresh_interval")
                .ok()
                .flatten()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3);

            crate::cache::Scheduler::spawn(
                ds_manager,
                cache,
                db,
                app.handle().clone(),
                interval,
            );

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::quote::get_quotes,
            commands::quote::get_indices,
            commands::watchlist::get_watchlist,
            commands::watchlist::add_watch,
            commands::watchlist::remove_watch,
            commands::watchlist::reorder_watch,
            commands::watchlist::search_stocks,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::settings::switch_datasource,
            commands::settings::list_datasources,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start application");
}
