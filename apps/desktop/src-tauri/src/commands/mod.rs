use batch_code_analyzer_app_core::{domain::ProjectId, ProjectService, ProjectServiceError};
use batch_code_analyzer_ipc_contracts::{
    DatabaseStatus, ErrorCategory, HealthCheckResponse, HealthStatus, IpcError, ProjectAddRequest,
    ProjectAddResponse, ProjectDetailDto, ProjectSummaryDto, HEALTH_CHECK_SCHEMA_VERSION,
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

#[tauri::command]
pub(crate) async fn project_list(
    state: State<'_, PersistenceState>,
) -> Result<Vec<ProjectSummaryDto>, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    ProjectService::new(database)
        .list_projects()
        .await
        .map(|projects| projects.iter().map(ProjectSummaryDto::from).collect())
        .map_err(|error| persistence_error(&error))
}

#[tauri::command]
pub(crate) async fn project_add(
    request: ProjectAddRequest,
    state: State<'_, PersistenceState>,
) -> Result<ProjectAddResponse, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let result = ProjectService::new(database)
        .add_project(request.source_directory)
        .await
        .map_err(project_service_error)?;
    Ok(ProjectAddResponse {
        project: ProjectDetailDto::from(&result.project),
        created: result.created,
        config_mirror_warning: result.config_mirror_warning,
    })
}

#[tauri::command]
pub(crate) async fn project_get(
    project_id: ProjectId,
    state: State<'_, PersistenceState>,
) -> Result<ProjectDetailDto, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let project = ProjectService::new(database)
        .get_project(&project_id)
        .await
        .map_err(|error| persistence_error(&error))?
        .ok_or_else(|| project_not_found(project_id.as_str()))?;
    Ok(ProjectDetailDto::from(&project))
}

fn database_unavailable() -> IpcError {
    ipc_error(
        "persistence_database_unavailable",
        ErrorCategory::Persistence,
        "本地数据库暂不可用",
        true,
    )
}

fn project_service_error(error: ProjectServiceError) -> IpcError {
    match error {
        ProjectServiceError::PathUnavailable => ipc_error(
            "project_path_unavailable",
            ErrorCategory::Project,
            "所选目录不可用",
            true,
        ),
        ProjectServiceError::Persistence(error) => persistence_error(&error),
    }
}

fn persistence_error(error: &batch_code_analyzer_persistence::PersistenceError) -> IpcError {
    let retryable = matches!(
        error,
        batch_code_analyzer_persistence::PersistenceError::DatabaseUnavailable
            | batch_code_analyzer_persistence::PersistenceError::TransactionFailed
    );
    let code = error.code();
    ipc_error(
        code,
        ErrorCategory::Persistence,
        "项目数据暂时无法保存",
        retryable,
    )
}

fn project_not_found(id: &str) -> IpcError {
    let _ = id;
    ipc_error(
        "project_not_found",
        ErrorCategory::Project,
        "项目不存在",
        false,
    )
}

fn ipc_error(
    code: &'static str,
    category: ErrorCategory,
    message: &'static str,
    retryable: bool,
) -> IpcError {
    IpcError {
        schema_version: 1,
        code: code.into(),
        category,
        message: message.into(),
        retryable,
        switch_profile: false,
        correlation_id: "project-command".into(),
        details: None,
    }
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
    use super::{health_check_response, ProjectServiceError};
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

    #[test]
    fn project_errors_use_stable_codes_and_safe_messages() {
        let path_error = super::project_service_error(ProjectServiceError::PathUnavailable);
        assert_eq!(path_error.code, "project_path_unavailable");
        assert_eq!(path_error.message, "所选目录不可用");
        assert!(path_error.details.is_none());

        let persistence_error = super::persistence_error(
            &batch_code_analyzer_persistence::PersistenceError::TransactionFailed,
        );
        assert_eq!(persistence_error.code, "persistence_transaction_failed");
        assert_eq!(persistence_error.message, "项目数据暂时无法保存");
        assert!(persistence_error.details.is_none());

        let unavailable = super::database_unavailable();
        assert_eq!(unavailable.code, "persistence_database_unavailable");
        assert_eq!(unavailable.message, "本地数据库暂不可用");
    }
}
