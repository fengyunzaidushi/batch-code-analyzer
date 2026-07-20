use batch_code_analyzer_persistence::{Database, DatabaseHealth, DatabaseStartup};
use batch_code_analyzer_secret_store::{MemorySecretStore, SecretStoreAvailability};
use std::sync::Arc;
use tauri::Manager;

mod commands;
mod scan_state;

pub(crate) use scan_state::ScanState;

pub(crate) struct PersistenceState {
    pub(crate) database: Option<Database>,
    pub(crate) health: DatabaseHealth,
    pub(crate) scans: ScanState,
    pub(crate) secret_store: Arc<MemorySecretStore>,
}

/// Starts the desktop application and registers its IPC commands.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or run the application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let database_path = app.path().app_data_dir()?.join("app.db");
            let startup = tauri::async_runtime::block_on(Database::open_for_startup(database_path));
            let state = match startup {
                DatabaseStartup::Ready(database) => PersistenceState {
                    health: database.health(),
                    database: Some(database),
                    scans: ScanState::default(),
                    secret_store: Arc::new(MemorySecretStore::with_availability(
                        SecretStoreAvailability::SessionOnly,
                    )),
                },
                DatabaseStartup::Recovery(recovery) => PersistenceState {
                    health: recovery.health(),
                    database: None,
                    scans: ScanState::default(),
                    secret_store: Arc::new(MemorySecretStore::with_availability(
                        SecretStoreAvailability::SessionOnly,
                    )),
                },
            };

            app.manage(state);
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
            commands::project_list,
            commands::project_add,
            commands::project_get,
            commands::project_update_run_settings,
            commands::context_generate,
            commands::context_get,
            commands::run_preview,
            commands::run_create,
            commands::run_execute,
            commands::api_profile_list,
            commands::api_profile_save,
            commands::api_profile_secret_put,
            commands::api_profile_test,
            commands::api_models_fetch,
            commands::api_profile_delete,
            commands::file_list,
            commands::file_set_included,
            commands::file_authorize_sensitive,
            commands::scan_start,
            commands::scan_cancel,
            commands::scan_get_report
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Batch Code Analyzer");
}
