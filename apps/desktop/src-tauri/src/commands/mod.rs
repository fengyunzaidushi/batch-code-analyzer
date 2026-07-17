use batch_code_analyzer_ipc_contracts::{
    DatabaseStatus, HealthCheckResponse, HealthStatus, HEALTH_CHECK_SCHEMA_VERSION,
};
use batch_code_analyzer_persistence::DatabaseHealth;
use tauri::State;

use crate::PersistenceState;

// Tauri's `CommandArg` implementation requires `State<T>` by value.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub(crate) fn health_check(state: State<'_, PersistenceState>) -> HealthCheckResponse {
    health_check_response(state.health)
}

fn health_check_response(database_health: DatabaseHealth) -> HealthCheckResponse {
    let (status, database_status, database_schema_version) = match database_health {
        DatabaseHealth::Ready { schema_version } => {
            (HealthStatus::Ready, DatabaseStatus::Ready, schema_version)
        }
        DatabaseHealth::MigrationFailed { schema_version } => (
            HealthStatus::Degraded,
            DatabaseStatus::MigrationFailed,
            schema_version,
        ),
        DatabaseHealth::Unavailable => (HealthStatus::Degraded, DatabaseStatus::Unavailable, 0),
    };

    HealthCheckResponse {
        schema_version: HEALTH_CHECK_SCHEMA_VERSION,
        status,
        app_version: env!("CARGO_PKG_VERSION").into(),
        database_status,
        database_schema_version,
    }
}

#[cfg(test)]
mod tests {
    use super::health_check_response;
    use batch_code_analyzer_persistence::DatabaseHealth;

    #[test]
    fn health_check_reports_typed_bootstrap_state() {
        let response = health_check_response(DatabaseHealth::Ready { schema_version: 1 });

        assert_eq!(
            response.status,
            batch_code_analyzer_ipc_contracts::HealthStatus::Ready
        );
        assert_eq!(response.app_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            response.database_status,
            batch_code_analyzer_ipc_contracts::DatabaseStatus::Ready
        );
        assert_eq!(response.database_schema_version, 1);
    }

    #[test]
    fn health_check_reports_migration_failure_without_driver_details() {
        let response = health_check_response(DatabaseHealth::MigrationFailed { schema_version: 1 });

        assert_eq!(
            response.status,
            batch_code_analyzer_ipc_contracts::HealthStatus::Degraded
        );
        assert_eq!(
            response.database_status,
            batch_code_analyzer_ipc_contracts::DatabaseStatus::MigrationFailed
        );
        assert_eq!(response.database_schema_version, 1);
    }

    #[test]
    fn health_check_reports_database_unavailability_as_degraded() {
        let response = health_check_response(DatabaseHealth::Unavailable);

        assert_eq!(
            response.status,
            batch_code_analyzer_ipc_contracts::HealthStatus::Degraded
        );
        assert_eq!(
            response.database_status,
            batch_code_analyzer_ipc_contracts::DatabaseStatus::Unavailable
        );
        assert_eq!(response.database_schema_version, 0);
    }
}
