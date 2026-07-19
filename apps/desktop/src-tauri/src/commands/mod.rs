use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use batch_code_analyzer_api_profiles::{
    ApiProfile as ProviderApiProfile, ApiProfileId as ProviderApiProfileId,
};
use batch_code_analyzer_app_core::{
    domain::{ApiModelInfo, ApiProfile, ApiProfileConnectionStatus, ApiProfileId, ProjectId},
    timestamp_now, ApiProfileService, ApiProfileServiceError, FileServiceError, ProjectService,
    ProjectServiceError, RunExecutionService, RunPreparationInput, RunService, RunServiceError,
    ScanService,
};
use batch_code_analyzer_ipc_contracts::{
    ApiModelsFetchRequest, ApiModelsFetchResponse, ApiProfileDeleteRequest,
    ApiProfileDeleteResponse, ApiProfileListResponse, ApiProfileSaveRequest,
    ApiProfileSaveResponse, ApiProfileSummaryDto, ApiProfileTestRequest, ApiProfileTestResponse,
    DatabaseStatus, ErrorCategory, FileListRequest, FileRecordSummaryDto, FileSetIncludedRequest,
    FileSetIncludedResponse, HealthCheckResponse, HealthStatus, IpcError, PageResponse,
    ProjectAddRequest, ProjectAddResponse, ProjectDetailDto, ProjectRunSettingsUpdateRequest,
    ProjectRunSettingsUpdateResponse, ProjectSummaryDto, RunBlockingReasonDto, RunCreateRequest,
    RunCreateResponse, RunExecuteRequest, RunExecuteResponse, RunPreviewRequest,
    RunPreviewResponse, RunPreviewTaskDto, RunSummaryDto, ScanCancelRequest, ScanCancelResponse,
    ScanOperationStatus, ScanReportDto, ScanRuleSummaryDto, ScanStartRequest, ScanStartResponse,
    DTO_SCHEMA_VERSION, HEALTH_CHECK_SCHEMA_VERSION,
};
use batch_code_analyzer_model_providers::{ModelProvider, OpenAiResponsesProvider, ProviderError};
use batch_code_analyzer_persistence::DatabaseHealth;
use batch_code_analyzer_repository_scanner::{ImportReport, ScanCancellation};
use batch_code_analyzer_secret_store::{SecretRef, SecretStore, SecretValue};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, State};

use crate::{scan_state::ScanStateError, PersistenceState};

static NEXT_SCAN_OPERATION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApiProfileSecretPutRequest {
    pub profile_id: ApiProfileId,
    pub secret: String,
}

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
pub(crate) async fn project_update_run_settings(
    request: ProjectRunSettingsUpdateRequest,
    state: State<'_, PersistenceState>,
) -> Result<ProjectRunSettingsUpdateResponse, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let result = ProjectService::new(database)
        .update_run_settings(
            &request.project_id,
            request.primary_profile_id,
            request.default_model,
        )
        .await
        .map_err(project_service_error)?;
    Ok(ProjectRunSettingsUpdateResponse {
        project: ProjectDetailDto::from(&result.project),
        config_mirror_warning: result.config_mirror_warning,
    })
}

#[tauri::command]
pub(crate) async fn run_preview(
    request: RunPreviewRequest,
    state: State<'_, PersistenceState>,
) -> Result<RunPreviewResponse, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let preview = RunService::new(database)
        .preview(
            &request.project_id,
            &RunPreparationInput {
                prompt: request.prompt,
                model: request.model,
            },
        )
        .await
        .map_err(run_service_error)?;
    Ok(RunPreviewResponse {
        schema_version: DTO_SCHEMA_VERSION,
        project_id: preview.project_id,
        tasks: preview
            .tasks
            .into_iter()
            .map(|task| RunPreviewTaskDto {
                file_id: task.file_id,
                relative_path: task.relative_path,
                size_bytes: task.size_bytes,
                content_hash: task.content_hash,
            })
            .collect(),
        blockers: preview
            .blockers
            .into_iter()
            .map(|blocker| RunBlockingReasonDto {
                code: blocker.code.to_owned(),
                message: blocker.message.to_owned(),
                file_id: blocker.file_id,
                relative_path: blocker.relative_path,
            })
            .collect(),
        model: preview.model,
        prompt_source: preview.prompt_source,
        model_source: preview.model_source,
        output_directory: preview.output_directory,
    })
}

#[tauri::command]
pub(crate) async fn run_create(
    request: RunCreateRequest,
    state: State<'_, PersistenceState>,
) -> Result<RunCreateResponse, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let run = RunService::new(database)
        .create(
            &request.project_id,
            &RunPreparationInput {
                prompt: request.prompt,
                model: request.model,
            },
        )
        .await
        .map_err(run_service_error)?;
    Ok(RunCreateResponse {
        task_count: run.stats.total,
        run: RunSummaryDto::from(&run),
    })
}

#[tauri::command]
pub(crate) async fn run_execute(
    request: RunExecuteRequest,
    state: State<'_, PersistenceState>,
) -> Result<RunExecuteResponse, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let provider = OpenAiResponsesProvider::new(state.secret_store.clone()).map_err(|_| {
        ipc_error(
            "provider_connection_failed",
            ErrorCategory::Provider,
            "模型 Provider 暂不可用",
            true,
        )
    })?;
    let run = RunExecutionService::new(database, provider)
        .execute(&request.run_id)
        .await
        .map_err(run_execution_error)?;
    Ok(RunExecuteResponse {
        run: RunSummaryDto::from(&run),
    })
}

#[tauri::command]
pub(crate) async fn api_profile_list(
    state: State<'_, PersistenceState>,
) -> Result<ApiProfileListResponse, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let profiles = ApiProfileService::new(database)
        .list()
        .await
        .map_err(|error| persistence_error(&error))?;
    let mut items = Vec::with_capacity(profiles.len());
    for profile in profiles {
        items.push(profile_summary(&state, &profile).await);
    }
    Ok(ApiProfileListResponse { items })
}

#[tauri::command]
pub(crate) async fn api_profile_save(
    request: ApiProfileSaveRequest,
    state: State<'_, PersistenceState>,
) -> Result<ApiProfileSaveResponse, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let profile = ApiProfileService::new(database)
        .save(
            request.id,
            request.name,
            request.base_url,
            request.default_model,
        )
        .await
        .map_err(api_profile_service_error)?;
    Ok(ApiProfileSaveResponse {
        profile: profile_summary(&state, &profile).await,
    })
}

#[tauri::command]
pub(crate) async fn api_profile_secret_put(
    request: ApiProfileSecretPutRequest,
    state: State<'_, PersistenceState>,
) -> Result<ApiProfileSaveResponse, IpcError> {
    if request.secret.trim().is_empty() {
        return Err(ipc_error(
            "validation_required_field",
            ErrorCategory::Validation,
            "API Key 不能为空",
            false,
        ));
    }
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let secret_ref = state
        .secret_store
        .put(SecretValue::new(request.secret))
        .await
        .map_err(secret_store_error)?;
    let profile = match ApiProfileService::new(database)
        .set_secret_ref(&request.profile_id, Some(secret_ref.as_str().to_owned()))
        .await
    {
        Ok(profile) => profile,
        Err(error) => {
            let _ = state.secret_store.delete(&secret_ref).await;
            return Err(api_profile_service_error(error));
        }
    };
    Ok(ApiProfileSaveResponse {
        profile: profile_summary(&state, &profile).await,
    })
}

#[tauri::command]
pub(crate) async fn api_profile_test(
    request: ApiProfileTestRequest,
    state: State<'_, PersistenceState>,
) -> Result<ApiProfileTestResponse, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let profile = ApiProfileService::new(database)
        .get(&request.id)
        .await
        .map_err(api_profile_service_error)?;
    let models = match fetch_models(&state, &profile).await {
        Ok(models) => models,
        Err(error) => {
            let _ = ApiProfileService::new(database)
                .set_test_result(
                    &profile.id,
                    ApiProfileConnectionStatus::Failed,
                    Some(error.code().to_owned()),
                    Vec::new(),
                )
                .await;
            return Err(provider_error(&error));
        }
    };
    let profile = ApiProfileService::new(database)
        .set_test_result(
            &profile.id,
            ApiProfileConnectionStatus::Healthy,
            None,
            models,
        )
        .await
        .map_err(api_profile_service_error)?;
    Ok(ApiProfileTestResponse {
        profile: profile_summary(&state, &profile).await,
    })
}

#[tauri::command]
pub(crate) async fn api_models_fetch(
    request: ApiModelsFetchRequest,
    state: State<'_, PersistenceState>,
) -> Result<ApiModelsFetchResponse, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let profile = ApiProfileService::new(database)
        .get(&request.id)
        .await
        .map_err(api_profile_service_error)?;
    let models = fetch_models(&state, &profile)
        .await
        .map_err(|error| provider_error(&error))?;
    let profile = ApiProfileService::new(database)
        .set_test_result(
            &profile.id,
            ApiProfileConnectionStatus::Healthy,
            None,
            models,
        )
        .await
        .map_err(api_profile_service_error)?;
    Ok(ApiModelsFetchResponse {
        profile: profile_summary(&state, &profile).await,
    })
}

#[tauri::command]
pub(crate) async fn api_profile_delete(
    request: ApiProfileDeleteRequest,
    state: State<'_, PersistenceState>,
) -> Result<ApiProfileDeleteResponse, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    ApiProfileService::new(database)
        .delete(&request.id)
        .await
        .map_err(api_profile_service_error)?;
    // Keep the opaque secret reference alive. It may be shared by another
    // profile; explicit orphan cleanup belongs to a later reference-counted task.
    Ok(ApiProfileDeleteResponse {
        id: request.id,
        deleted: true,
    })
}

async fn profile_summary(
    state: &State<'_, PersistenceState>,
    profile: &ApiProfile,
) -> ApiProfileSummaryDto {
    let has_secret = match profile.secret_ref.as_deref() {
        Some(reference) => state
            .secret_store
            .get(&SecretRef::new(reference))
            .await
            .is_ok(),
        None => false,
    };
    ApiProfileSummaryDto::from_profile(profile, has_secret)
}

async fn fetch_models(
    state: &State<'_, PersistenceState>,
    profile: &ApiProfile,
) -> Result<Vec<ApiModelInfo>, ProviderError> {
    let secret_ref = profile
        .secret_ref
        .as_deref()
        .ok_or(ProviderError::SecretStoreUnavailable)?;
    let provider_profile = ProviderApiProfile::new(
        ProviderApiProfileId::new(profile.id.to_string()),
        profile.name.clone(),
        profile.base_url.clone(),
        SecretRef::new(secret_ref),
    )
    .map_err(|_| ProviderError::InvalidRequest { status: 400 })?;
    let provider = OpenAiResponsesProvider::new(state.secret_store.clone())
        .map_err(|_| ProviderError::ConnectionFailed)?;
    let models = provider.list_models(&provider_profile.resolve()).await?;
    Ok(models
        .into_iter()
        .map(|model| ApiModelInfo {
            id: model.id,
            display_name: model.display_name,
            owned_by: model.owned_by,
        })
        .collect())
}

fn provider_error(error: &ProviderError) -> IpcError {
    let category = if error.code().starts_with("security_") {
        ErrorCategory::Security
    } else {
        ErrorCategory::Provider
    };
    ipc_error_with_switch(
        error.code(),
        category,
        "API Profile 连接测试失败",
        error.retryable(),
        error.switch_profile(),
    )
}

fn secret_store_error(error: batch_code_analyzer_secret_store::SecretError) -> IpcError {
    ipc_error(
        error.code(),
        ErrorCategory::Security,
        "安全存储暂不可用",
        matches!(
            error,
            batch_code_analyzer_secret_store::SecretError::Unavailable
                | batch_code_analyzer_secret_store::SecretError::BackendFailure
        ),
    )
}

fn api_profile_service_error(error: ApiProfileServiceError) -> IpcError {
    match error {
        ApiProfileServiceError::NotFound => ipc_error(
            "validation_invalid_value",
            ErrorCategory::Validation,
            "API Profile 不存在",
            false,
        ),
        ApiProfileServiceError::InvalidName | ApiProfileServiceError::InvalidBaseUrl => ipc_error(
            "validation_invalid_value",
            ErrorCategory::Validation,
            "API Profile 配置无效",
            false,
        ),
        ApiProfileServiceError::UrlContainsCredentials => ipc_error(
            "security_invalid_secret_reference",
            ErrorCategory::Security,
            "Base URL 不得包含凭据",
            false,
        ),
        ApiProfileServiceError::Persistence(error) => {
            if matches!(
                error,
                batch_code_analyzer_persistence::PersistenceError::StateTransition {
                    code: "api_profile_in_use"
                }
            ) {
                ipc_error(
                    "api_profile_in_use",
                    ErrorCategory::Project,
                    "API Profile 仍被项目使用",
                    false,
                )
            } else if matches!(
                error,
                batch_code_analyzer_persistence::PersistenceError::StateTransition {
                    code: "api_profile_name_duplicate"
                }
            ) {
                ipc_error(
                    "api_profile_name_duplicate",
                    ErrorCategory::Validation,
                    "API Profile 名称已存在",
                    false,
                )
            } else {
                persistence_error(&error)
            }
        }
    }
}

#[tauri::command]
pub(crate) async fn file_list(
    request: FileListRequest,
    state: State<'_, PersistenceState>,
) -> Result<PageResponse<FileRecordSummaryDto>, IpcError> {
    if !(1..=500).contains(&request.limit) {
        return Err(ipc_error(
            "validation_limit_exceeded",
            ErrorCategory::Validation,
            "文件列表分页大小无效",
            false,
        ));
    }
    let offset = request
        .cursor
        .as_deref()
        .map(str::parse::<usize>)
        .transpose()
        .map_err(|_| {
            ipc_error(
                "validation_invalid_value",
                ErrorCategory::Validation,
                "文件列表游标无效",
                false,
            )
        })?
        .unwrap_or(0);
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let project = ProjectService::new(database)
        .get_project(&request.project_id)
        .await
        .map_err(|error| persistence_error(&error))?
        .ok_or_else(|| project_not_found(request.project_id.as_str()))?;
    let _ = project;
    let records = ProjectService::new(database)
        .list_file_records(&request.project_id)
        .await
        .map_err(|error| persistence_error(&error))?;
    let total = u32::try_from(records.len()).unwrap_or(u32::MAX);
    let limit = usize::from(request.limit);
    let items: Vec<FileRecordSummaryDto> = records
        .iter()
        .skip(offset)
        .take(limit)
        .map(FileRecordSummaryDto::from)
        .collect();
    let next_offset = offset.saturating_add(items.len());
    let next_cursor = (next_offset < records.len()).then(|| next_offset.to_string());
    Ok(PageResponse {
        items,
        next_cursor,
        total,
    })
}

#[tauri::command]
pub(crate) async fn file_set_included(
    request: FileSetIncludedRequest,
    state: State<'_, PersistenceState>,
) -> Result<FileSetIncludedResponse, IpcError> {
    let database = state.database.as_ref().ok_or_else(database_unavailable)?;
    let file = ProjectService::new(database)
        .set_file_included(&request.project_id, &request.file_id, request.included)
        .await
        .map_err(file_service_error)?;
    Ok(FileSetIncludedResponse {
        file: FileRecordSummaryDto::from(&file),
    })
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
    let temporary_patterns = request
        .temporary_excluded_patterns
        .unwrap_or_default()
        .into_iter()
        .map(|pattern| pattern.trim().to_owned())
        .filter(|pattern| !pattern.is_empty())
        .take(100)
        .collect::<Vec<_>>();
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
        let temporary_patterns_for_scan = temporary_patterns.clone();
        let scan_result = tauri::async_runtime::spawn_blocking(move || {
            ScanService::scan_project_with_patterns(
                &project_for_scan,
                cancellation_for_scan,
                temporary_patterns_for_scan,
            )
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
        rules: ScanRuleSummaryDto {
            builtin_directories: report.builtin_directories.clone(),
            builtin_extensions: report.builtin_extensions.clone(),
            gitignore_rules: report.gitignore_rules.clone(),
            temporary_excluded_patterns: report.temporary_excluded_patterns.clone(),
            sensitive_detection_enabled: report.sensitive_detection_enabled,
        },
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
        ProjectServiceError::NotFound => project_not_found(""),
        ProjectServiceError::ApiProfileNotFound => ipc_error(
            "validation_invalid_value",
            ErrorCategory::Validation,
            "主 API Profile 不存在",
            false,
        ),
        ProjectServiceError::PathUnavailable => ipc_error(
            "project_path_unavailable",
            ErrorCategory::Project,
            "所选目录不可用",
            true,
        ),
        ProjectServiceError::Persistence(error) => persistence_error(&error),
    }
}

fn run_service_error(error: RunServiceError) -> IpcError {
    match error {
        RunServiceError::NotFound => project_not_found(""),
        RunServiceError::ActiveRun => ipc_error(
            "run_active_exists",
            ErrorCategory::Scheduler,
            "当前已有活动 Run",
            false,
        ),
        RunServiceError::Blocked(reason) => {
            let category = if reason.code.starts_with("security_") {
                ErrorCategory::Security
            } else if reason.code.starts_with("project_") {
                ErrorCategory::Project
            } else if reason.code.starts_with("run_") || reason.code.starts_with("task_") {
                ErrorCategory::Scheduler
            } else {
                ErrorCategory::Validation
            };
            ipc_error(reason.code, category, reason.message, false)
        }
        RunServiceError::Persistence(error) => persistence_error(&error),
    }
}

fn run_execution_error(error: batch_code_analyzer_app_core::RunExecutionError) -> IpcError {
    match error {
        batch_code_analyzer_app_core::RunExecutionError::NotFound => ipc_error(
            "run_not_found",
            ErrorCategory::Scheduler,
            "Run 不存在",
            false,
        ),
        batch_code_analyzer_app_core::RunExecutionError::NotRunning => ipc_error(
            "run_not_active",
            ErrorCategory::Scheduler,
            "Run 当前不可执行",
            false,
        ),
        batch_code_analyzer_app_core::RunExecutionError::PathUnavailable => ipc_error(
            "project_path_unavailable",
            ErrorCategory::Project,
            "项目路径不可用",
            true,
        ),
        batch_code_analyzer_app_core::RunExecutionError::OutputWriteFailed => ipc_error(
            "output_write_failed",
            ErrorCategory::Output,
            "分析结果暂时无法写入",
            true,
        ),
        batch_code_analyzer_app_core::RunExecutionError::Persistence(error) => {
            persistence_error(&error)
        }
    }
}

fn file_service_error(error: FileServiceError) -> IpcError {
    match error {
        FileServiceError::NotFound | FileServiceError::Deleted => ipc_error(
            "validation_invalid_value",
            ErrorCategory::Validation,
            "文件记录不存在或已删除",
            false,
        ),
        FileServiceError::SensitiveBlocked => ipc_error(
            "security_sensitive_file_blocked",
            ErrorCategory::Security,
            "敏感文件需要单独确认后才能纳入",
            false,
        ),
        FileServiceError::Unreadable => ipc_error(
            "scan_file_unreadable",
            ErrorCategory::Scan,
            "文件不可读取",
            true,
        ),
        FileServiceError::UnsupportedEncoding => ipc_error(
            "scan_encoding_unsupported",
            ErrorCategory::Scan,
            "文件编码不支持",
            false,
        ),
        FileServiceError::Binary => ipc_error(
            "scan_binary_file",
            ErrorCategory::Scan,
            "二进制文件不能纳入分析",
            false,
        ),
        FileServiceError::TooLarge => ipc_error(
            "scan_file_too_large",
            ErrorCategory::Scan,
            "文件超过大小限制",
            false,
        ),
        FileServiceError::RuleExcluded => ipc_error(
            "validation_invalid_value",
            ErrorCategory::Validation,
            "文件被扫描规则排除",
            false,
        ),
        FileServiceError::Persistence(error) => persistence_error(&error),
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
    ipc_error_with_switch(code, category, message, retryable, false)
}

fn ipc_error_with_switch(
    code: &'static str,
    category: ErrorCategory,
    message: &'static str,
    retryable: bool,
    switch_profile: bool,
) -> IpcError {
    IpcError {
        schema_version: 1,
        code: code.into(),
        category,
        message: message.into(),
        retryable,
        switch_profile,
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
    use super::{
        health_check_response, ApiProfileServiceError, FileServiceError, ProjectServiceError,
        ProviderError, RunServiceError,
    };
    use batch_code_analyzer_app_core::RunBlockingReason;
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

    #[test]
    fn file_errors_use_stable_codes_and_safe_messages() {
        let sensitive = super::file_service_error(FileServiceError::SensitiveBlocked);
        assert_eq!(sensitive.code, "security_sensitive_file_blocked");
        assert_eq!(sensitive.message, "敏感文件需要单独确认后才能纳入");
        assert!(sensitive.details.is_none());

        let missing = super::file_service_error(FileServiceError::NotFound);
        assert_eq!(missing.code, "validation_invalid_value");
        assert_eq!(missing.message, "文件记录不存在或已删除");
        assert!(missing.details.is_none());
    }

    #[test]
    fn api_profile_errors_use_stable_codes_and_never_include_secrets() {
        let invalid =
            super::api_profile_service_error(ApiProfileServiceError::UrlContainsCredentials);
        assert_eq!(invalid.code, "security_invalid_secret_reference");
        assert_eq!(invalid.message, "Base URL 不得包含凭据");
        assert!(invalid.details.is_none());

        let provider = super::provider_error(&ProviderError::AuthenticationFailed { status: 401 });
        assert_eq!(provider.code, "provider_authentication_failed");
        assert_eq!(provider.message, "API Profile 连接测试失败");
        assert!(provider.details.is_none());
        assert!(!provider.message.contains("401"));
    }

    #[test]
    fn run_errors_use_scheduler_and_validation_categories() {
        let active = super::run_service_error(RunServiceError::ActiveRun);
        assert_eq!(active.code, "run_active_exists");
        assert_eq!(
            active.category,
            batch_code_analyzer_ipc_contracts::ErrorCategory::Scheduler
        );

        let blocked = super::run_service_error(RunServiceError::Blocked(RunBlockingReason {
            code: "validation_model_missing",
            message: "无法解析任务实际模型",
            file_id: None,
            relative_path: None,
        }));
        assert_eq!(blocked.code, "validation_model_missing");
        assert_eq!(
            blocked.category,
            batch_code_analyzer_ipc_contracts::ErrorCategory::Validation
        );
        assert!(blocked.details.is_none());
    }
}
