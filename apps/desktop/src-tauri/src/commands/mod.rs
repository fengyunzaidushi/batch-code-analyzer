use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use batch_code_analyzer_app_core::{
    domain::ProjectId, timestamp_now, ProjectService, ProjectServiceError, ScanService,
};
use batch_code_analyzer_ipc_contracts::{
    DatabaseStatus, ErrorCategory, HealthCheckResponse, HealthStatus, IpcError, ProjectAddRequest,
    ProjectAddResponse, ProjectDetailDto, ProjectSummaryDto, ScanCancelRequest, ScanCancelResponse,
    ScanOperationStatus, ScanReportDto, ScanStartRequest, ScanStartResponse,
    HEALTH_CHECK_SCHEMA_VERSION,
};
use batch_code_analyzer_persistence::DatabaseHealth;
use batch_code_analyzer_repository_scanner::{ImportReport, ScanCancellation};
use tauri::{AppHandle, Emitter, State};

use crate::{scan_state::ScanStateError, PersistenceState};

static NEXT_SCAN_OPERATION_ID: AtomicU64 = AtomicU64::new(1);

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

#[tauri::command]
#[allow(clippy::too_many_lines)]
pub(crate) async fn scan_start(
    request: ScanStartRequest,
    state: State<'_, PersistenceState>,
    app: AppHandle,
) -> Result<ScanStartResponse, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let project = ProjectService::new(database)
        .get_project(&request.project_id)
        .await
        .map_err(|error| persistence_error(&error))?
        .ok_or_else(|| project_not_found(request.project_id.as_str()))?;
    let operation_id = new_scan_operation_id();
    let cancellation = ScanCancellation::new();
    let report = scan_report(
        &operation_id,
        &project.id,
        ScanOperationStatus::Running,
        &ImportReport::default(),
        None,
        None,
        None,
    );
    state
        .scans
        .begin(
            operation_id.clone(),
            project.id.clone(),
            cancellation.clone(),
            report.clone(),
        )
        .map_err(scan_state_error)?;
    emit_scan_progress(&app, &report);

    let scans = state.scans.clone();
    let database = database.clone();
    let operation_id_for_task = operation_id.clone();
    let project_id_for_task = project.id.clone();
    let project_id_for_response = project.id.clone();
    tauri::async_runtime::spawn(async move {
        let project_for_scan = project.clone();
        let cancellation_for_scan = cancellation.clone();
        let scan_result = tauri::async_runtime::spawn_blocking(move || {
            ScanService::scan_project(&project_for_scan, cancellation_for_scan)
        })
        .await;
        let report = match scan_result {
            Ok(Ok(result)) if !result.completed => scan_report(
                &operation_id_for_task,
                &project_id_for_task,
                ScanOperationStatus::Cancelled,
                &result.report,
                None,
                None,
                Some("scan_cancelled"),
            ),
            Ok(Ok(result)) => match ScanService::new(&database)
                .persist_scan(&project_id_for_task, result)
                .await
            {
                Ok(summary) => scan_report(
                    &operation_id_for_task,
                    &project_id_for_task,
                    ScanOperationStatus::Completed,
                    &summary.report,
                    Some(summary.file_count),
                    Some(summary.generation),
                    None,
                ),
                Err(error) => scan_report(
                    &operation_id_for_task,
                    &project_id_for_task,
                    ScanOperationStatus::Failed,
                    &ImportReport::default(),
                    None,
                    None,
                    Some(error.code()),
                ),
            },
            Ok(Err(error)) => scan_report(
                &operation_id_for_task,
                &project_id_for_task,
                ScanOperationStatus::Failed,
                &ImportReport::default(),
                None,
                None,
                Some(error.code()),
            ),
            Err(_) => scan_report(
                &operation_id_for_task,
                &project_id_for_task,
                ScanOperationStatus::Failed,
                &ImportReport::default(),
                None,
                None,
                Some("scan_failed"),
            ),
        };
        let _ = scans.update(&operation_id_for_task, report.clone());
        emit_scan_progress(&app, &report);
    });

    Ok(ScanStartResponse {
        schema_version: 1,
        operation_id,
        project_id: project_id_for_response,
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn scan_cancel(
    request: ScanCancelRequest,
    state: State<'_, PersistenceState>,
) -> Result<ScanCancelResponse, IpcError> {
    let accepted = state
        .scans
        .cancel(&request.operation_id)
        .map_err(scan_state_error)?;
    Ok(ScanCancelResponse {
        schema_version: 1,
        operation_id: request.operation_id,
        accepted,
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn scan_get_report(
    operation_id: String,
    state: State<'_, PersistenceState>,
) -> Result<ScanReportDto, IpcError> {
    state.scans.report(&operation_id).map_err(scan_state_error)
}

fn emit_scan_progress(app: &AppHandle, report: &ScanReportDto) {
    let _ = app.emit("scan://progress", report);
}

fn scan_report(
    operation_id: &str,
    project_id: &ProjectId,
    status: ScanOperationStatus,
    report: &ImportReport,
    file_count: Option<u32>,
    generation: Option<u32>,
    error_code: Option<&str>,
) -> ScanReportDto {
    ScanReportDto {
        schema_version: 1,
        operation_id: operation_id.into(),
        project_id: project_id.clone(),
        status,
        visited_entries: report.visited_entries,
        scanned_files: report.scanned_files,
        included_files: report.included_files,
        excluded_by_reason: report.excluded_by_reason.clone(),
        unreadable_files: report.unreadable_files.clone(),
        unsupported_encoding_files: report.unsupported_encoding_files.clone(),
        sensitive_files: report.sensitive_files.clone(),
        symlink_files: report.symlink_files.clone(),
        invalid_gitignore_rules: report.invalid_gitignore_rules.clone(),
        cancelled: report.cancelled,
        file_count,
        generation,
        error_code: error_code.map(str::to_owned),
        updated_at: timestamp_now(),
    }
}

fn new_scan_operation_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = NEXT_SCAN_OPERATION_ID.fetch_add(1, Ordering::Relaxed);
    format!("scan-{timestamp}-{sequence}")
}

fn scan_state_error(error: ScanStateError) -> IpcError {
    match error {
        ScanStateError::AlreadyRunning => ipc_error(
            "scan_already_running",
            ErrorCategory::Scan,
            "当前项目已有扫描",
            false,
        ),
        ScanStateError::NotFound => ipc_error(
            "scan_not_found",
            ErrorCategory::Scan,
            "扫描操作不存在",
            false,
        ),
    }
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
