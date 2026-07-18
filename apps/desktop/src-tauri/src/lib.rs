use batch_code_analyzer_persistence::{Database, DatabaseHealth, DatabaseStartup};
use tauri::Manager;

mod commands;

pub(crate) struct PersistenceState {
    pub(crate) database: Option<Database>,
    pub(crate) health: DatabaseHealth,
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
                },
                DatabaseStartup::Recovery(recovery) => PersistenceState {
                    health: recovery.health(),
                    database: None,
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
            commands::project_get
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Batch Code Analyzer");
}
