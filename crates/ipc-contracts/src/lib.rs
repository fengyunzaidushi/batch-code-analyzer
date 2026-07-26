//! Stable DTOs shared across the IPC boundary.

#![forbid(unsafe_code)]

use std::{collections::BTreeMap, path::Path};

use batch_code_analyzer_domain::{
    ApiModelInfo, ApiProfile, ApiProfileConnectionStatus, ApiProfileId, ApiProtocol,
    Attempt as DomainAttempt, AttemptError as DomainAttemptError, AttemptId, AttemptStatus,
    ContextStatus, ContextVersion as DomainContextVersion, ContextVersionId,
    FileRecord as DomainFileRecord, FileRecordId, FileResultStatus, FileSourceStatus,
    Project as DomainProject, ProjectId, ProjectPathStatus, PromptPreset as DomainPromptPreset,
    Rfc3339Timestamp, Run as DomainRun, RunId, RunStats, RunStatus, RunTransition,
    Task as DomainTask, TaskId, TaskStatus, TaskTransition, TaskValueSource,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::{Config, ExportError, TS};

pub const HEALTH_CHECK_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Validation,
    Project,
    Scan,
    Security,
    Persistence,
    Provider,
    Scheduler,
    Output,
    Recovery,
    Internal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub code: String,
    pub category: ErrorCategory,
    pub message: String,
    pub retryable: bool,
    pub switch_profile: bool,
    pub correlation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "Record<string, unknown>")]
    pub details: Option<BTreeMap<String, Value>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PageRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cursor: Option<String>,
    pub limit: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PageResponse<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<String>,
    pub total: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ready,
    Degraded,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseStatus {
    NotInitialized,
    Ready,
    MigrationFailed,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResponse {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub status: HealthStatus,
    pub app_version: String,
    pub database_status: DatabaseStatus,
    pub database_schema_version: u32,
}

impl HealthCheckResponse {
    #[must_use]
    pub fn ready(app_version: impl Into<String>) -> Self {
        Self {
            schema_version: HEALTH_CHECK_SCHEMA_VERSION,
            status: HealthStatus::Ready,
            app_version: app_version.into(),
            database_status: DatabaseStatus::NotInitialized,
            database_schema_version: 0,
        }
    }
}

pub const DTO_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfileSummaryDto {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub id: ApiProfileId,
    pub name: String,
    pub protocol: ApiProtocol,
    pub base_url: String,
    pub default_model: Option<String>,
    pub model_cache: Vec<ApiModelInfo>,
    pub model_cache_updated_at: Option<Rfc3339Timestamp>,
    pub has_secret: bool,
    pub last_connection_status: ApiProfileConnectionStatus,
    pub last_error_code: Option<String>,
    pub last_tested_at: Option<Rfc3339Timestamp>,
}

impl ApiProfileSummaryDto {
    #[must_use]
    pub fn from_profile(profile: &ApiProfile, has_secret: bool) -> Self {
        Self {
            schema_version: DTO_SCHEMA_VERSION,
            id: profile.id.clone(),
            name: profile.name.clone(),
            protocol: profile.protocol,
            base_url: profile.base_url.clone(),
            default_model: profile.default_model.clone(),
            model_cache: profile.model_cache.clone(),
            model_cache_updated_at: profile.model_cache_updated_at.clone(),
            has_secret,
            last_connection_status: profile.last_connection_status,
            last_error_code: profile.last_error_code.clone(),
            last_tested_at: profile.last_tested_at.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfileSaveRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub id: Option<ApiProfileId>,
    pub name: String,
    pub base_url: String,
    pub default_model: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfileSaveResponse {
    pub profile: ApiProfileSummaryDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfileListResponse {
    pub items: Vec<ApiProfileSummaryDto>,
}

/// One-shot response used only after an explicit reveal action. The value must
/// not be logged, cached, persisted, or included in ordinary profile DTOs.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfileSecretGetRequest {
    pub profile_id: ApiProfileId,
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfileSecretGetResponse {
    pub secret: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfileTestRequest {
    pub id: ApiProfileId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfileTestResponse {
    pub profile: ApiProfileSummaryDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfileDeleteRequest {
    pub id: ApiProfileId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfileDeleteResponse {
    pub id: ApiProfileId,
    pub deleted: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiModelsFetchRequest {
    pub id: ApiProfileId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiModelsFetchResponse {
    pub profile: ApiProfileSummaryDto,
}

/// UI-safe project list item. The repository's absolute location remains in
/// the Rust application layer and is never exposed as a list DTO field.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummaryDto {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub id: ProjectId,
    pub name: String,
    pub path_status: ProjectPathStatus,
    pub last_opened_at: Rfc3339Timestamp,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAddRequest {
    pub source_directory: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PromptPresetDto {
    pub id: String,
    pub name: String,
    pub prompt: String,
}

/// Detail DTO intentionally exposes the selected project's path only on demand.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetailDto {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub id: ProjectId,
    pub name: String,
    pub source_directory: String,
    pub path_status: ProjectPathStatus,
    pub default_prompt: String,
    pub prompt_presets: Vec<PromptPresetDto>,
    pub active_prompt_id: Option<String>,
    pub default_model: Option<String>,
    pub context_model: Option<String>,
    pub api_routing: batch_code_analyzer_domain::ApiRouting,
    pub concurrency: u16,
    pub output_root: Option<String>,
    pub last_opened_at: Rfc3339Timestamp,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAddResponse {
    pub project: ProjectDetailDto,
    pub created: bool,
    pub config_mirror_warning: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRunSettingsUpdateRequest {
    pub project_id: ProjectId,
    pub primary_profile_id: Option<ApiProfileId>,
    pub default_model: Option<String>,
    pub concurrency: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRunSettingsUpdateResponse {
    pub project: ProjectDetailDto,
    pub config_mirror_warning: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPromptSaveRequest {
    pub project_id: ProjectId,
    pub name: String,
    pub prompt: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPromptSaveResponse {
    pub project: ProjectDetailDto,
    pub config_mirror_warning: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPromptSelectRequest {
    pub project_id: ProjectId,
    pub prompt_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPromptSelectResponse {
    pub project: ProjectDetailDto,
    pub config_mirror_warning: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppDataResetRequest {
    pub confirmed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppDataResetResponse {
    pub scheduled: bool,
    pub restart_required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileListRequest {
    pub project_id: ProjectId,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cursor: Option<String>,
    pub limit: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileSetIncludedRequest {
    pub project_id: ProjectId,
    pub file_id: FileRecordId,
    pub included: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileAuthorizeSensitiveRequest {
    pub project_id: ProjectId,
    pub file_id: FileRecordId,
    pub confirmed: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ScanOperationStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScanStartRequest {
    pub project_id: ProjectId,
    #[serde(default)]
    #[ts(optional)]
    pub temporary_excluded_patterns: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScanStartResponse {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub operation_id: String,
    pub project_id: ProjectId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScanCancelRequest {
    pub operation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScanCancelResponse {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub operation_id: String,
    pub accepted: bool,
}

/// Scan progress and the final import report share one stable payload so the
/// UI can render the latest operation without retaining scanner internals.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScanReportDto {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub operation_id: String,
    pub project_id: ProjectId,
    pub status: ScanOperationStatus,
    #[ts(type = "number")]
    pub visited_entries: u64,
    #[ts(type = "number")]
    pub scanned_files: u64,
    #[ts(type = "number")]
    pub included_files: u64,
    #[ts(type = "Record<string, number>")]
    pub excluded_by_reason: BTreeMap<String, u64>,
    pub unreadable_files: Vec<String>,
    pub unsupported_encoding_files: Vec<String>,
    pub sensitive_files: Vec<String>,
    pub symlink_files: Vec<String>,
    pub invalid_gitignore_rules: Vec<String>,
    pub rules: ScanRuleSummaryDto,
    pub cancelled: bool,
    pub file_count: Option<u32>,
    pub generation: Option<u32>,
    pub error_code: Option<String>,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScanRuleSummaryDto {
    pub builtin_directories: Vec<String>,
    pub builtin_extensions: Vec<String>,
    pub gitignore_rules: Vec<String>,
    pub temporary_excluded_patterns: Vec<String>,
    pub sensitive_detection_enabled: bool,
}

impl From<&DomainProject> for ProjectSummaryDto {
    fn from(project: &DomainProject) -> Self {
        Self {
            schema_version: DTO_SCHEMA_VERSION,
            id: project.id.clone(),
            name: project.name.clone(),
            path_status: project.path_status,
            last_opened_at: project.last_opened_at.clone(),
        }
    }
}

impl From<&DomainProject> for ProjectDetailDto {
    fn from(project: &DomainProject) -> Self {
        Self::with_prompt_presets(project, &project.filter_rules.prompt_presets)
    }
}

impl ProjectDetailDto {
    /// Builds a project detail DTO with the client-wide prompt library supplied
    /// by the application layer. The domain project retains legacy presets only
    /// for backwards-compatible data import.
    #[must_use]
    pub fn with_prompt_presets(
        project: &DomainProject,
        prompt_presets: &[DomainPromptPreset],
    ) -> Self {
        Self {
            schema_version: DTO_SCHEMA_VERSION,
            id: project.id.clone(),
            name: project.name.clone(),
            source_directory: project.source_directory.clone(),
            path_status: project.path_status,
            default_prompt: project.default_prompt.clone(),
            prompt_presets: prompt_presets.iter().map(PromptPresetDto::from).collect(),
            active_prompt_id: project.filter_rules.active_prompt_id.clone(),
            default_model: project.default_model.clone(),
            context_model: project.context_model.clone(),
            api_routing: project.api_routing.clone(),
            concurrency: project.execution_defaults.concurrency,
            output_root: project.output_root.clone(),
            last_opened_at: project.last_opened_at.clone(),
        }
    }
}

impl From<&DomainPromptPreset> for PromptPresetDto {
    fn from(preset: &DomainPromptPreset) -> Self {
        Self {
            id: preset.id.clone(),
            name: preset.name.clone(),
            prompt: preset.prompt.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileRecordSummaryDto {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub id: FileRecordId,
    pub project_id: ProjectId,
    pub relative_path: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
    pub modified_at: Option<Rfc3339Timestamp>,
    pub language: Option<String>,
    pub source_status: FileSourceStatus,
    pub included: bool,
    pub exclusion_reason: Option<String>,
    pub result_status: FileResultStatus,
}

impl From<&DomainFileRecord> for FileRecordSummaryDto {
    fn from(file_record: &DomainFileRecord) -> Self {
        Self {
            schema_version: DTO_SCHEMA_VERSION,
            id: file_record.id.clone(),
            project_id: file_record.project_id.clone(),
            relative_path: file_record.relative_path.clone(),
            size_bytes: file_record.size_bytes,
            modified_at: file_record.modified_at.clone(),
            language: file_record.language.clone(),
            source_status: file_record.source_status,
            included: file_record.included,
            exclusion_reason: file_record.exclusion_reason.clone(),
            result_status: file_record.result_status,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileSetIncludedResponse {
    pub file: FileRecordSummaryDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileAuthorizeSensitiveResponse {
    pub file: FileRecordSummaryDto,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunStatsDto {
    pub total: u32,
    pub pending: u32,
    pub queued: u32,
    pub running: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub cancelled: u32,
    pub interrupted: u32,
    pub source_changed: u32,
}

impl From<&RunStats> for RunStatsDto {
    fn from(stats: &RunStats) -> Self {
        Self {
            total: stats.total,
            pending: stats.pending,
            queued: stats.queued,
            running: stats.running,
            succeeded: stats.succeeded,
            failed: stats.failed,
            cancelled: stats.cancelled,
            interrupted: stats.interrupted,
            source_changed: stats.source_changed,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunSummaryDto {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub id: RunId,
    pub project_id: ProjectId,
    pub status: RunStatus,
    pub created_at: Rfc3339Timestamp,
    pub started_at: Option<Rfc3339Timestamp>,
    pub completed_at: Option<Rfc3339Timestamp>,
    pub context_version_id: Option<ContextVersionId>,
    pub stats: RunStatsDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunPreviewRequest {
    pub project_id: ProjectId,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunCreateRequest {
    pub project_id: ProjectId,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunBlockingReasonDto {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub file_id: Option<FileRecordId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub relative_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunPreviewTaskDto {
    pub file_id: FileRecordId,
    pub relative_path: String,
    pub size_bytes: u64,
    pub content_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunPreviewResponse {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub project_id: ProjectId,
    pub tasks: Vec<RunPreviewTaskDto>,
    pub blockers: Vec<RunBlockingReasonDto>,
    pub model: Option<String>,
    pub prompt_source: TaskValueSource,
    pub model_source: TaskValueSource,
    pub concurrency: u16,
    pub output_directory: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunCreateResponse {
    pub run: RunSummaryDto,
    pub task_count: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunExecuteRequest {
    pub run_id: RunId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunExecuteResponse {
    pub run: RunSummaryDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunCancelRequest {
    pub run_id: RunId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunCancelResponse {
    pub run: RunSummaryDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunListRequest {
    pub project_id: ProjectId,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cursor: Option<String>,
    pub limit: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunGetRequest {
    pub project_id: ProjectId,
    pub run_id: RunId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunGetResponse {
    pub run: RunSummaryDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskListRequest {
    pub project_id: ProjectId,
    pub run_id: RunId,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cursor: Option<String>,
    pub limit: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskGetRequest {
    pub project_id: ProjectId,
    pub task_id: TaskId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskGetResponse {
    pub task: TaskSummaryDto,
    pub prompt_snapshot: String,
    pub attempts: Vec<AttemptDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequestPreviewRequest {
    pub project_id: ProjectId,
    pub task_id: TaskId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequestPreviewResponse {
    pub task: TaskSummaryDto,
    pub instructions: String,
    pub input: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskRetryRequest {
    pub project_id: ProjectId,
    pub task_id: TaskId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskRetryResponse {
    pub run: RunSummaryDto,
    pub task: TaskSummaryDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskRetryBatchRequest {
    pub project_id: ProjectId,
    pub run_id: RunId,
    pub task_ids: Vec<TaskId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskRetryBatchResponse {
    pub run: RunSummaryDto,
    pub retried_task_ids: Vec<TaskId>,
    pub skipped_task_ids: Vec<TaskId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResultReadRequest {
    pub project_id: ProjectId,
    pub task_id: TaskId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResultReadResponse {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub project_id: ProjectId,
    pub run_id: RunId,
    pub task_id: TaskId,
    pub relative_path: String,
    pub result_version: u32,
    pub markdown: String,
}

impl From<&DomainRun> for RunSummaryDto {
    fn from(run: &DomainRun) -> Self {
        Self {
            schema_version: DTO_SCHEMA_VERSION,
            id: run.id.clone(),
            project_id: run.project_id.clone(),
            status: run.status,
            created_at: run.created_at.clone(),
            started_at: run.started_at.clone(),
            completed_at: run.completed_at.clone(),
            context_version_id: run.context_version_id.clone(),
            stats: RunStatsDto::from(&run.stats),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummaryDto {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub id: TaskId,
    pub run_id: RunId,
    pub file_id: FileRecordId,
    pub relative_path: String,
    pub status: TaskStatus,
    pub prompt_source: TaskValueSource,
    pub model_snapshot: String,
    pub model_source: TaskValueSource,
    pub has_result: bool,
    pub result_version: u32,
    pub latest_attempt_id: Option<AttemptId>,
    pub created_at: Rfc3339Timestamp,
    pub started_at: Option<Rfc3339Timestamp>,
    pub completed_at: Option<Rfc3339Timestamp>,
}

impl From<&DomainTask> for TaskSummaryDto {
    fn from(task: &DomainTask) -> Self {
        Self {
            schema_version: DTO_SCHEMA_VERSION,
            id: task.id.clone(),
            run_id: task.run_id.clone(),
            file_id: task.file_id.clone(),
            relative_path: task.relative_path.clone(),
            status: task.status,
            prompt_source: task.prompt_source,
            model_snapshot: task.model_snapshot.clone(),
            model_source: task.model_source,
            has_result: task.current_result_path.is_some(),
            result_version: task.result_version,
            latest_attempt_id: task.latest_attempt_id.clone(),
            created_at: task.created_at.clone(),
            started_at: task.started_at.clone(),
            completed_at: task.completed_at.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AttemptErrorDto {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub sanitized: bool,
}

impl From<&DomainAttemptError> for AttemptErrorDto {
    fn from(error: &DomainAttemptError) -> Self {
        Self {
            code: error.code.clone(),
            message: error.message.clone(),
            retryable: error.retryable,
            sanitized: error.sanitized,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AttemptDto {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub id: AttemptId,
    pub task_id: TaskId,
    pub sequence: u32,
    pub api_profile_id: ApiProfileId,
    pub api_profile_name: String,
    pub actual_model: String,
    pub status: AttemptStatus,
    pub started_at: Option<Rfc3339Timestamp>,
    pub finished_at: Option<Rfc3339Timestamp>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    pub http_status: Option<u16>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub retry_reason: Option<String>,
    pub error: Option<AttemptErrorDto>,
}

impl From<&DomainAttempt> for AttemptDto {
    fn from(attempt: &DomainAttempt) -> Self {
        Self {
            schema_version: DTO_SCHEMA_VERSION,
            id: attempt.id.clone(),
            task_id: attempt.task_id.clone(),
            sequence: attempt.sequence,
            api_profile_id: attempt.api_profile_id.clone(),
            api_profile_name: attempt.api_profile_name.clone(),
            actual_model: attempt.actual_model.clone(),
            status: attempt.status,
            started_at: attempt.started_at.clone(),
            finished_at: attempt.finished_at.clone(),
            duration_ms: attempt.duration_ms,
            http_status: attempt.http_status,
            input_tokens: attempt.input_tokens,
            output_tokens: attempt.output_tokens,
            total_tokens: attempt.total_tokens,
            retry_reason: attempt.retry_reason.clone(),
            error: attempt.error.as_ref().map(AttemptErrorDto::from),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContextVersionSourceFileDto {
    pub relative_path: String,
    pub content_hash: String,
    pub included: bool,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContextVersionDto {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub id: ContextVersionId,
    pub project_id: ProjectId,
    pub status: ContextStatus,
    pub source_files: Vec<ContextVersionSourceFileDto>,
    pub model: Option<String>,
    pub summary: String,
    pub summary_hash: String,
    pub manually_edited: bool,
    pub created_at: Rfc3339Timestamp,
}

impl From<&DomainContextVersion> for ContextVersionDto {
    fn from(context_version: &DomainContextVersion) -> Self {
        Self {
            schema_version: DTO_SCHEMA_VERSION,
            id: context_version.id.clone(),
            project_id: context_version.project_id.clone(),
            status: context_version.status,
            source_files: context_version
                .source_files
                .iter()
                .map(|source_file| ContextVersionSourceFileDto {
                    relative_path: source_file.relative_path.clone(),
                    content_hash: source_file.content_hash.clone(),
                    included: source_file.included,
                    truncated: source_file.truncated,
                })
                .collect(),
            model: context_version.model.clone(),
            summary: context_version.summary.clone(),
            summary_hash: context_version.summary_hash.clone(),
            manually_edited: context_version.manually_edited,
            created_at: context_version.created_at.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContextGenerateRequest {
    pub project_id: ProjectId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContextGenerateResponse {
    pub context: ContextVersionDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContextGetRequest {
    pub project_id: ProjectId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContextGetResponse {
    pub context: Option<ContextVersionDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PromptGenerateRequest {
    pub project_id: ProjectId,
    pub goal: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PromptGenerateResponse {
    pub prompt: String,
}

/// Exports all currently stable Rust DTOs to individual TypeScript modules.
///
/// # Errors
///
/// Returns a `ts-rs` error when the configured output directory cannot be
/// created or a generated declaration cannot be written.
pub fn export_types(out_dir: &Path) -> Result<(), ExportError> {
    let config = Config::new().with_out_dir(out_dir);

    IpcError::export_all(&config)?;
    ApiProfileSummaryDto::export_all(&config)?;
    ApiProfileSaveRequest::export_all(&config)?;
    ApiProfileSaveResponse::export_all(&config)?;
    ApiProfileListResponse::export_all(&config)?;
    ApiProfileSecretGetRequest::export_all(&config)?;
    ApiProfileSecretGetResponse::export_all(&config)?;
    ApiProfileTestRequest::export_all(&config)?;
    ApiProfileTestResponse::export_all(&config)?;
    ApiProfileDeleteRequest::export_all(&config)?;
    ApiProfileDeleteResponse::export_all(&config)?;
    ApiModelsFetchRequest::export_all(&config)?;
    ApiModelsFetchResponse::export_all(&config)?;
    PageRequest::export_all(&config)?;
    PageResponse::<String>::export_all(&config)?;
    HealthCheckResponse::export_all(&config)?;
    ProjectSummaryDto::export_all(&config)?;
    PromptPresetDto::export_all(&config)?;
    ProjectAddRequest::export_all(&config)?;
    ProjectDetailDto::export_all(&config)?;
    ProjectAddResponse::export_all(&config)?;
    ProjectRunSettingsUpdateRequest::export_all(&config)?;
    ProjectRunSettingsUpdateResponse::export_all(&config)?;
    ProjectPromptSaveRequest::export_all(&config)?;
    ProjectPromptSaveResponse::export_all(&config)?;
    ProjectPromptSelectRequest::export_all(&config)?;
    ProjectPromptSelectResponse::export_all(&config)?;
    AppDataResetRequest::export_all(&config)?;
    AppDataResetResponse::export_all(&config)?;
    ScanStartRequest::export_all(&config)?;
    ScanStartResponse::export_all(&config)?;
    ScanCancelRequest::export_all(&config)?;
    ScanCancelResponse::export_all(&config)?;
    ScanReportDto::export_all(&config)?;
    ScanRuleSummaryDto::export_all(&config)?;
    FileListRequest::export_all(&config)?;
    FileSetIncludedRequest::export_all(&config)?;
    FileSetIncludedResponse::export_all(&config)?;
    FileAuthorizeSensitiveRequest::export_all(&config)?;
    FileAuthorizeSensitiveResponse::export_all(&config)?;
    FileRecordSummaryDto::export_all(&config)?;
    RunSummaryDto::export_all(&config)?;
    RunPreviewRequest::export_all(&config)?;
    RunPreviewResponse::export_all(&config)?;
    RunCreateRequest::export_all(&config)?;
    RunCreateResponse::export_all(&config)?;
    RunExecuteRequest::export_all(&config)?;
    RunExecuteResponse::export_all(&config)?;
    RunCancelRequest::export_all(&config)?;
    RunCancelResponse::export_all(&config)?;
    RunListRequest::export_all(&config)?;
    RunGetRequest::export_all(&config)?;
    RunGetResponse::export_all(&config)?;
    RunBlockingReasonDto::export_all(&config)?;
    RunPreviewTaskDto::export_all(&config)?;
    TaskSummaryDto::export_all(&config)?;
    TaskListRequest::export_all(&config)?;
    TaskGetRequest::export_all(&config)?;
    TaskGetResponse::export_all(&config)?;
    TaskRequestPreviewRequest::export_all(&config)?;
    TaskRequestPreviewResponse::export_all(&config)?;
    TaskRetryRequest::export_all(&config)?;
    TaskRetryResponse::export_all(&config)?;
    TaskRetryBatchRequest::export_all(&config)?;
    TaskRetryBatchResponse::export_all(&config)?;
    AttemptDto::export_all(&config)?;
    ResultReadRequest::export_all(&config)?;
    ResultReadResponse::export_all(&config)?;
    ContextVersionDto::export_all(&config)?;
    ContextGenerateRequest::export_all(&config)?;
    ContextGenerateResponse::export_all(&config)?;
    ContextGetRequest::export_all(&config)?;
    ContextGetResponse::export_all(&config)?;
    PromptGenerateRequest::export_all(&config)?;
    PromptGenerateResponse::export_all(&config)?;
    ProjectId::export_all(&config)?;
    FileRecordId::export_all(&config)?;
    RunId::export_all(&config)?;
    TaskId::export_all(&config)?;
    AttemptId::export_all(&config)?;
    ContextVersionId::export_all(&config)?;
    ApiProfileId::export_all(&config)?;
    Rfc3339Timestamp::export_all(&config)?;
    ProjectPathStatus::export_all(&config)?;
    FileSourceStatus::export_all(&config)?;
    FileResultStatus::export_all(&config)?;
    ContextStatus::export_all(&config)?;
    TaskValueSource::export_all(&config)?;
    RunStatus::export_all(&config)?;
    RunTransition::export_all(&config)?;
    TaskStatus::export_all(&config)?;
    TaskTransition::export_all(&config)?;
    AttemptStatus::export_all(&config)?;
    ScanOperationStatus::export_all(&config)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::{
        export_types, ApiProfileSummaryDto, HealthCheckResponse, HealthStatus, ProjectSummaryDto,
        HEALTH_CHECK_SCHEMA_VERSION,
    };
    use batch_code_analyzer_domain::{
        ApiProfile, ApiProfileConnectionStatus, ApiProfileId, ApiProtocol, ApiRouting,
        ContextStatus, ExecutionDefaults, FilterRules, Project as DomainProject, ProjectContext,
        ProjectId, ProjectPathStatus, Rfc3339Timestamp,
    };

    #[test]
    fn health_check_response_reports_bootstrap_database_state() {
        let response = HealthCheckResponse::ready("0.1.0");

        assert_eq!(response.schema_version, HEALTH_CHECK_SCHEMA_VERSION);
        assert_eq!(response.status, HealthStatus::Ready);
        assert_eq!(response.database_schema_version, 0);
    }

    #[test]
    fn type_export_includes_the_health_check_contract() {
        let out_dir = env::temp_dir().join(format!(
            "batch-code-analyzer-ipc-types-{}",
            std::process::id()
        ));

        if out_dir.exists() {
            fs::remove_dir_all(&out_dir).expect("temporary output directory should be removable");
        }

        export_types(&out_dir).expect("TypeScript DTO generation should succeed");

        let health_check = fs::read_to_string(out_dir.join("HealthCheckResponse.ts"))
            .expect("health check declaration should be generated");
        assert!(health_check.contains("HealthCheckResponse"));
        assert!(health_check.contains("schemaVersion: 1"));

        let task_summary = fs::read_to_string(out_dir.join("TaskSummaryDto.ts"))
            .expect("task summary declaration should be generated");
        assert!(task_summary.contains("TaskSummaryDto"));
        assert!(task_summary.contains("TaskValueSource"));
        assert!(task_summary.contains("hasResult: boolean"));
        assert!(!task_summary.contains("currentResultPath"));

        let task_get = fs::read_to_string(out_dir.join("TaskGetResponse.ts"))
            .expect("task detail declaration should be generated");
        assert!(task_get.contains("promptSnapshot: string"));
        assert!(!task_get.contains("input: string"));

        let request_preview = fs::read_to_string(out_dir.join("TaskRequestPreviewResponse.ts"))
            .expect("request preview declaration should be generated");
        assert!(request_preview.contains("instructions: string"));
        assert!(request_preview.contains("input: string"));

        let file_summary = fs::read_to_string(out_dir.join("FileRecordSummaryDto.ts"))
            .expect("file summary declaration should be generated");
        assert!(file_summary.contains("sizeBytes: number"));

        let attempt = fs::read_to_string(out_dir.join("AttemptDto.ts"))
            .expect("attempt declaration should be generated");
        assert!(attempt.contains("durationMs: number | null"));

        fs::remove_dir_all(&out_dir).expect("temporary output directory should be removable");
    }

    #[test]
    fn project_summary_is_a_ui_safe_contract() {
        let project = DomainProject {
            schema_version: 2,
            id: ProjectId::new("project-1"),
            name: "example".into(),
            source_directory: "/private/workspace/example".into(),
            path_status: ProjectPathStatus::Available,
            default_prompt: "Explain this file".into(),
            default_model: Some("gpt-5".into()),
            context_model: None,
            api_routing: ApiRouting {
                primary_profile_id: Some(ApiProfileId::new("profile-1")),
                fallbacks: Vec::new(),
            },
            execution_defaults: ExecutionDefaults {
                concurrency: 5,
                timeout_seconds: 120,
                max_output_tokens: 4096,
                retry_count_per_profile: 1,
            },
            project_context: ProjectContext {
                enabled: true,
                current_version_id: None,
                status: ContextStatus::Ready,
            },
            filter_rules: FilterRules::default(),
            output_root: Some("/private/workspace/results".into()),
            last_opened_at: Rfc3339Timestamp::new("2026-07-15T10:38:25+08:00"),
        };
        let dto = ProjectSummaryDto::from(&project);

        let serialized = serde_json::to_string(&dto).expect("DTO should serialize");
        assert!(serialized.contains("schemaVersion"));
        assert!(serialized.contains("pathStatus"));
        assert!(!serialized.contains("sourceDirectory"));
        assert!(!serialized.contains("/private/workspace"));
        assert!(!serialized.contains("apiKey"));
    }

    #[test]
    fn api_profile_summary_never_serializes_secret_reference() {
        let profile = ApiProfile {
            id: ApiProfileId::new("profile-1"),
            name: "Local".into(),
            protocol: ApiProtocol::OpenAiResponses,
            base_url: "https://example.test/v1".into(),
            secret_ref: Some("session-secret-1".into()),
            default_model: None,
            model_cache: Vec::new(),
            model_cache_updated_at: None,
            last_connection_status: ApiProfileConnectionStatus::Unknown,
            last_error_code: None,
            last_tested_at: None,
            created_at: Rfc3339Timestamp::new("2026-07-18T10:00:00Z"),
            updated_at: Rfc3339Timestamp::new("2026-07-18T10:00:00Z"),
        };
        let json = serde_json::to_string(&ApiProfileSummaryDto::from_profile(&profile, true))
            .expect("summary should serialize");
        assert!(json.contains("hasSecret"));
        assert!(!json.contains("session-secret-1"));
        assert!(!json.contains("secretRef"));
    }
}
