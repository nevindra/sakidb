mod commands;
mod state;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex;

use dashmap::DashMap;
use sakidb_postgres::PostgresDriver;
use sakidb_store::Store;
use state::AppState;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let data_dir = dirs::data_dir()
        .expect("Could not determine data directory")
        .join("sakidb");
    std::fs::create_dir_all(&data_dir).expect("Could not create data directory");
    let db_path = data_dir.join("config.db");

    info!(data_dir = %data_dir.display(), "starting SakiDB");

    let store = Store::open(db_path.to_str().expect("Invalid path"))
        .expect("Could not open config store");

    info!("config store opened");

    let app_state = AppState {
        driver: Arc::new(PostgresDriver::new()),
        store: Arc::new(Mutex::new(store)),
        restore_cancelled: Arc::new(AtomicBool::new(false)),
        export_cancel_flags: Arc::new(DashMap::new()),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(app_state)
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                use tauri::{TitleBarStyle, WebviewUrl, WebviewWindowBuilder};
                use cocoa::appkit::{NSColor, NSWindow};
                use cocoa::base::{id, nil};

                let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                    .title("SakiDB")
                    .inner_size(1200.0, 800.0)
                    .min_inner_size(800.0, 600.0)
                    .hidden_title(true)
                    .title_bar_style(TitleBarStyle::Transparent)
                    .build()?;

                // Match title bar color to app background #1a1a1e
                unsafe {
                    let ns_window = window.ns_window()? as id;
                    let color = NSColor::colorWithRed_green_blue_alpha_(
                        nil,
                        26.0 / 255.0,
                        26.0 / 255.0,
                        30.0 / 255.0,
                        1.0,
                    );
                    ns_window.setBackgroundColor_(color);
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                use tauri::{WebviewUrl, WebviewWindowBuilder};
                WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                    .title("SakiDB")
                    .inner_size(1200.0, 800.0)
                    .min_inner_size(800.0, 600.0)
                    .decorations(false)
                    .build()?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connection::save_connection,
            commands::connection::list_connections,
            commands::connection::delete_connection,
            commands::connection::update_connection,
            commands::connection::test_connection,
            commands::connection::connect_to_database,
            commands::connection::connect_to_database_as,
            commands::connection::disconnect_from_database,
            commands::connection::drop_database,
            commands::connection::create_database,
            commands::connection::rename_database,
            commands::connection::update_last_connected,
            commands::query::execute_query,
            commands::query::execute_query_multi,
            commands::query::execute_query_multi_columnar,
            commands::query::execute_query_paged,
            commands::query::execute_batch,
            commands::query::cancel_query,
            commands::explorer::list_databases,
            commands::explorer::list_schemas,
            commands::explorer::list_tables,
            commands::explorer::list_columns,
            commands::explorer::list_views,
            commands::explorer::list_materialized_views,
            commands::explorer::list_functions,
            commands::explorer::list_sequences,
            commands::explorer::list_indexes,
            commands::explorer::list_foreign_tables,
            commands::explorer::list_triggers,
            commands::explorer::list_foreign_keys,
            commands::explorer::list_check_constraints,
            commands::explorer::list_unique_constraints,
            commands::explorer::get_partition_info,
            commands::explorer::get_create_table_sql,
            commands::explorer::get_erd_data,
            commands::explorer::get_schema_completion_data,
            commands::explorer::get_completion_bundle,
            commands::explorer::get_table_columns_for_completion,
            commands::export::export_table_csv,
            commands::export::export_table_sql,
            commands::export::cancel_export,
            commands::import::restore_from_sql,
            commands::import::cancel_restore,
            commands::queries::save_query,
            commands::queries::list_saved_queries,
            commands::queries::update_saved_query,
            commands::queries::delete_saved_query,
            commands::queries::add_query_history,
            commands::queries::list_query_history,
            commands::queries::clear_query_history,
            commands::queries::save_from_history,
            commands::settings::get_keybinding_overrides,
            commands::settings::set_keybinding,
            commands::settings::reset_keybinding,
            commands::settings::reset_all_keybindings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SakiDB");
}
