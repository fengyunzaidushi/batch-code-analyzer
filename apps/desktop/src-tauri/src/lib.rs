use batch_code_analyzer_persistence::{Database, DatabaseHealth, DatabaseStartup};
use tauri::Manager;

mod commands;

pub(crate) struct PersistenceState {
    _database: Option<Database>,
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
                    _database: Some(database),
                },
                DatabaseStartup::Recovery(recovery) => PersistenceState {
                    health: recovery.health(),
                    _database: None,
                },
            };

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::health_check])
        .run(tauri::generate_context!())
        .expect("failed to run Batch Code Analyzer");
}
