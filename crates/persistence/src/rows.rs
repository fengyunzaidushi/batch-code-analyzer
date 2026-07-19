//! `SQLite` row representations and lossless conversions to domain entities.

use batch_code_analyzer_domain::{
    ApiProfile, ApiProfileId, Attempt, AttemptError, AttemptId, ContextVersion, ContextVersionId,
    FileRecord, FileRecordId, Project, ProjectContext, ProjectId, Rfc3339Timestamp, Run, RunId,
    Task, TaskId,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::PersistenceError;

#[derive(Debug)]
pub struct ProjectRow {
    pub id: String,
    pub schema_version: i64,
    pub name: String,
    pub source_directory: String,
    pub canonical_source_directory: String,
    pub path_status: String,
    pub default_prompt: String,
    pub default_model: Option<String>,
    pub context_model: Option<String>,
    pub output_root: Option<String>,
    pub filter_rules_json: String,
    pub execution_defaults_json: String,
    pub api_routing_json: String,
    pub current_context_version_id: Option<String>,
    pub context_enabled: bool,
    pub context_status: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_opened_at: String,
}

pub struct ProjectRowMetadata {
    pub canonical_source_directory: String,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

impl ProjectRow {
    /// Converts a domain `Project` to its database row plus persistence metadata.
    ///
    /// # Errors
    ///
    /// Returns `internal_contract_violation` if an entity cannot be serialized
    /// into its documented JSON-backed columns.
    pub fn from_domain(
        project: &Project,
        metadata: ProjectRowMetadata,
    ) -> Result<Self, PersistenceError> {
        Ok(Self {
            id: project.id.to_string(),
            schema_version: i64::from(project.schema_version),
            name: project.name.clone(),
            source_directory: project.source_directory.clone(),
            canonical_source_directory: metadata.canonical_source_directory,
            path_status: encode(project.path_status)?,
            default_prompt: project.default_prompt.clone(),
            default_model: project.default_model.clone(),
            context_model: project.context_model.clone(),
            output_root: project.output_root.clone(),
            filter_rules_json: encode(project.filter_rules.clone())?,
            execution_defaults_json: encode(project.execution_defaults.clone())?,
            api_routing_json: encode(project.api_routing.clone())?,
            current_context_version_id: project
                .project_context
                .current_version_id
                .as_ref()
                .map(ToString::to_string),
            context_enabled: project.project_context.enabled,
            context_status: encode(project.project_context.status)?,
            created_at: timestamp_string(&metadata.created_at),
            updated_at: timestamp_string(&metadata.updated_at),
            last_opened_at: timestamp_string(&project.last_opened_at),
        })
    }
}

impl TryFrom<ProjectRow> for Project {
    type Error = PersistenceError;

    fn try_from(row: ProjectRow) -> Result<Self, Self::Error> {
        Ok(Self {
            schema_version: as_u32(row.schema_version)?,
            id: ProjectId::from(row.id),
            name: row.name,
            source_directory: row.source_directory,
            path_status: decode(&row.path_status)?,
            default_prompt: row.default_prompt,
            default_model: row.default_model,
            context_model: row.context_model,
            api_routing: decode(&row.api_routing_json)?,
            execution_defaults: decode(&row.execution_defaults_json)?,
            project_context: ProjectContext {
                enabled: row.context_enabled,
                current_version_id: row.current_context_version_id.map(ContextVersionId::from),
                status: decode(&row.context_status)?,
            },
            filter_rules: decode(&row.filter_rules_json)?,
            output_root: row.output_root,
            last_opened_at: Rfc3339Timestamp::new(row.last_opened_at),
        })
    }
}

#[derive(Debug)]
pub struct ApiProfileRow {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub key_reference_id: Option<String>,
    pub default_model: Option<String>,
    pub model_cache_json: String,
    pub model_cache_updated_at: Option<String>,
    pub last_connection_status: String,
    pub last_error_code: Option<String>,
    pub last_tested_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct ApiProfileRowMetadata {
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

impl ApiProfileRow {
    /// Converts API profile domain metadata to its `SQLite` row representation.
    ///
    /// # Errors
    ///
    /// Returns `internal_contract_violation` when cached metadata cannot be encoded.
    pub fn from_domain(
        profile: &ApiProfile,
        metadata: &ApiProfileRowMetadata,
    ) -> Result<Self, PersistenceError> {
        Ok(Self {
            id: profile.id.to_string(),
            name: profile.name.clone(),
            protocol: encode(profile.protocol)?,
            base_url: profile.base_url.clone(),
            key_reference_id: profile.secret_ref.clone(),
            default_model: profile.default_model.clone(),
            model_cache_json: encode(profile.model_cache.clone())?,
            model_cache_updated_at: profile
                .model_cache_updated_at
                .as_ref()
                .map(timestamp_string),
            last_connection_status: encode(profile.last_connection_status)?,
            last_error_code: profile.last_error_code.clone(),
            last_tested_at: profile.last_tested_at.as_ref().map(timestamp_string),
            created_at: timestamp_string(&metadata.created_at),
            updated_at: timestamp_string(&metadata.updated_at),
        })
    }
}

impl TryFrom<ApiProfileRow> for ApiProfile {
    type Error = PersistenceError;

    fn try_from(row: ApiProfileRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: ApiProfileId::from(row.id),
            name: row.name,
            protocol: decode(&row.protocol)?,
            base_url: row.base_url,
            secret_ref: row.key_reference_id,
            default_model: row.default_model,
            model_cache: decode(&row.model_cache_json)?,
            model_cache_updated_at: row.model_cache_updated_at.map(Rfc3339Timestamp::new),
            last_connection_status: decode(&row.last_connection_status)?,
            last_error_code: row.last_error_code,
            last_tested_at: row.last_tested_at.map(Rfc3339Timestamp::new),
            created_at: Rfc3339Timestamp::new(row.created_at),
            updated_at: Rfc3339Timestamp::new(row.updated_at),
        })
    }
}

#[derive(Debug)]
pub struct FileRecordRow {
    pub id: String,
    pub project_id: String,
    pub relative_path: String,
    pub normalized_relative_path: String,
    pub size_bytes: i64,
    pub modified_at: Option<String>,
    pub content_hash: Option<String>,
    pub encoding: Option<String>,
    pub language: Option<String>,
    pub source_status: String,
    pub included: bool,
    pub exclusion_reason: Option<String>,
    pub sensitive_findings_json: String,
    pub result_status: String,
    pub latest_successful_run_id: Option<String>,
    pub latest_successful_task_id: Option<String>,
    pub scan_generation: i64,
    pub created_at: String,
    pub updated_at: String,
}

pub struct FileRecordRowMetadata {
    pub normalized_relative_path: String,
    pub latest_successful_task_id: Option<TaskId>,
    pub scan_generation: u32,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

impl FileRecordRow {
    /// Converts a domain `FileRecord` to its database row plus scanner metadata.
    ///
    /// # Errors
    ///
    /// Returns `internal_contract_violation` for values `SQLite` cannot store in
    /// its documented integer or JSON columns.
    pub fn from_domain(
        file_record: &FileRecord,
        metadata: FileRecordRowMetadata,
    ) -> Result<Self, PersistenceError> {
        Ok(Self {
            id: file_record.id.to_string(),
            project_id: file_record.project_id.to_string(),
            relative_path: file_record.relative_path.clone(),
            normalized_relative_path: metadata.normalized_relative_path,
            size_bytes: i64::try_from(file_record.size_bytes)
                .map_err(|_| PersistenceError::InvalidStoredState)?,
            modified_at: file_record.modified_at.as_ref().map(timestamp_string),
            content_hash: file_record.content_hash.clone(),
            encoding: file_record.encoding.clone(),
            language: file_record.language.clone(),
            source_status: encode(file_record.source_status)?,
            included: file_record.included,
            exclusion_reason: file_record.exclusion_reason.clone(),
            sensitive_findings_json: encode(file_record.sensitive_findings.clone())?,
            result_status: encode(file_record.result_status)?,
            latest_successful_run_id: file_record
                .latest_successful_run_id
                .as_ref()
                .map(ToString::to_string),
            latest_successful_task_id: metadata.latest_successful_task_id.map(|id| id.to_string()),
            scan_generation: i64::from(metadata.scan_generation),
            created_at: timestamp_string(&metadata.created_at),
            updated_at: timestamp_string(&metadata.updated_at),
        })
    }
}

impl TryFrom<FileRecordRow> for FileRecord {
    type Error = PersistenceError;

    fn try_from(row: FileRecordRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: FileRecordId::from(row.id),
            project_id: ProjectId::from(row.project_id),
            relative_path: row.relative_path,
            size_bytes: u64::try_from(row.size_bytes)
                .map_err(|_| PersistenceError::InvalidStoredState)?,
            modified_at: row.modified_at.map(Rfc3339Timestamp::new),
            content_hash: row.content_hash,
            encoding: row.encoding,
            language: row.language,
            source_status: decode(&row.source_status)?,
            included: row.included,
            exclusion_reason: row.exclusion_reason,
            sensitive_findings: decode(&row.sensitive_findings_json)?,
            latest_successful_run_id: row.latest_successful_run_id.map(RunId::from),
            result_status: decode(&row.result_status)?,
        })
    }
}

#[derive(Debug)]
pub struct ContextVersionRow {
    pub id: String,
    pub project_id: String,
    pub status: String,
    pub model: Option<String>,
    pub source_files_json: String,
    pub summary: String,
    pub summary_hash: String,
    pub manually_edited: bool,
    pub created_at: String,
}

impl From<&ContextVersion> for ContextVersionRow {
    fn from(context_version: &ContextVersion) -> Self {
        Self {
            id: context_version.id.to_string(),
            project_id: context_version.project_id.to_string(),
            status: encode(context_version.status).expect("domain context status is serializable"),
            model: context_version.model.clone(),
            source_files_json: encode(context_version.source_files.clone())
                .expect("domain source files are serializable"),
            summary: context_version.summary.clone(),
            summary_hash: context_version.summary_hash.clone(),
            manually_edited: context_version.manually_edited,
            created_at: timestamp_string(&context_version.created_at),
        }
    }
}

impl TryFrom<ContextVersionRow> for ContextVersion {
    type Error = PersistenceError;

    fn try_from(row: ContextVersionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: ContextVersionId::from(row.id),
            project_id: ProjectId::from(row.project_id),
            status: decode(&row.status)?,
            source_files: decode(&row.source_files_json)?,
            model: row.model,
            summary: row.summary,
            summary_hash: row.summary_hash,
            manually_edited: row.manually_edited,
            created_at: Rfc3339Timestamp::new(row.created_at),
        })
    }
}

#[derive(Debug)]
pub struct RunRow {
    pub id: String,
    pub project_id: String,
    pub status: String,
    pub context_version_id: Option<String>,
    pub output_directory: String,
    pub snapshot_json: String,
    pub stats_json: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub interruption_reason: Option<String>,
}

pub struct RunRowMetadata {
    pub interruption_reason: Option<String>,
}

impl RunRow {
    /// Converts a domain Run to its database row. Snapshot JSON is serialized
    /// once here and protected by a database trigger after insertion.
    ///
    /// # Errors
    ///
    /// Returns `internal_contract_violation` if the immutable snapshot or
    /// statistics cannot be serialized.
    pub fn from_domain(run: &Run, metadata: RunRowMetadata) -> Result<Self, PersistenceError> {
        Ok(Self {
            id: run.id.to_string(),
            project_id: run.project_id.to_string(),
            status: encode(run.status)?,
            context_version_id: run.context_version_id.as_ref().map(ToString::to_string),
            output_directory: run.output_directory.clone(),
            snapshot_json: encode(run.snapshot.clone())?,
            stats_json: encode(run.stats.clone())?,
            created_at: timestamp_string(&run.created_at),
            started_at: run.started_at.as_ref().map(timestamp_string),
            completed_at: run.completed_at.as_ref().map(timestamp_string),
            interruption_reason: metadata.interruption_reason,
        })
    }
}

impl TryFrom<RunRow> for Run {
    type Error = PersistenceError;

    fn try_from(row: RunRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: RunId::from(row.id),
            project_id: ProjectId::from(row.project_id),
            status: decode(&row.status)?,
            created_at: Rfc3339Timestamp::new(row.created_at),
            started_at: row.started_at.map(Rfc3339Timestamp::new),
            completed_at: row.completed_at.map(Rfc3339Timestamp::new),
            context_version_id: row.context_version_id.map(ContextVersionId::from),
            output_directory: row.output_directory,
            snapshot: decode(&row.snapshot_json)?,
            stats: decode(&row.stats_json)?,
        })
    }
}

#[derive(Debug)]
pub struct TaskRow {
    pub id: String,
    pub run_id: String,
    pub file_id: String,
    pub relative_path: String,
    pub file_snapshot_json: String,
    pub prompt_snapshot: String,
    pub prompt_hash: String,
    pub prompt_source: String,
    pub model_snapshot: String,
    pub model_source: String,
    pub context_version_id: Option<String>,
    pub status: String,
    pub current_result_path: Option<String>,
    pub latest_attempt_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub result_version: i64,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

impl From<&Task> for TaskRow {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id.to_string(),
            run_id: task.run_id.to_string(),
            file_id: task.file_id.to_string(),
            relative_path: task.relative_path.clone(),
            file_snapshot_json: encode(task.file_snapshot.clone())
                .expect("domain file snapshot is serializable"),
            prompt_snapshot: task.prompt_snapshot.clone(),
            prompt_hash: task.prompt_hash.clone(),
            prompt_source: encode(task.prompt_source)
                .expect("domain prompt source is serializable"),
            model_snapshot: task.model_snapshot.clone(),
            model_source: encode(task.model_source).expect("domain model source is serializable"),
            context_version_id: task.context_version_id.as_ref().map(ToString::to_string),
            status: encode(task.status).expect("domain task status is serializable"),
            current_result_path: task.current_result_path.clone(),
            latest_attempt_id: task.latest_attempt_id.as_ref().map(ToString::to_string),
            parent_task_id: task.parent_task_id.as_ref().map(ToString::to_string),
            result_version: i64::from(task.result_version),
            created_at: timestamp_string(&task.created_at),
            started_at: task.started_at.as_ref().map(timestamp_string),
            completed_at: task.completed_at.as_ref().map(timestamp_string),
        }
    }
}

impl TryFrom<TaskRow> for Task {
    type Error = PersistenceError;

    fn try_from(row: TaskRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: TaskId::from(row.id),
            run_id: RunId::from(row.run_id),
            file_id: FileRecordId::from(row.file_id),
            relative_path: row.relative_path,
            file_snapshot: decode(&row.file_snapshot_json)?,
            prompt_snapshot: row.prompt_snapshot,
            prompt_hash: row.prompt_hash,
            prompt_source: decode(&row.prompt_source)?,
            model_snapshot: row.model_snapshot,
            model_source: decode(&row.model_source)?,
            context_version_id: row.context_version_id.map(ContextVersionId::from),
            status: decode(&row.status)?,
            current_result_path: row.current_result_path,
            latest_attempt_id: row.latest_attempt_id.map(AttemptId::from),
            parent_task_id: row.parent_task_id.map(TaskId::from),
            result_version: as_u32(row.result_version)?,
            created_at: Rfc3339Timestamp::new(row.created_at),
            started_at: row.started_at.map(Rfc3339Timestamp::new),
            completed_at: row.completed_at.map(Rfc3339Timestamp::new),
        })
    }
}

#[derive(Debug)]
pub struct AttemptRow {
    pub id: String,
    pub task_id: String,
    pub sequence: i64,
    pub api_profile_id: String,
    pub api_profile_name_snapshot: String,
    pub actual_model: String,
    pub status: String,
    pub created_at: String,
    pub request_started_at: Option<String>,
    pub request_dispatched_at: Option<String>,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub http_status: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub retry_reason: Option<String>,
    pub error_code: Option<String>,
    pub sanitized_error_message: Option<String>,
    pub error_retryable: Option<bool>,
    pub error_sanitized: Option<bool>,
    pub response_id: Option<String>,
}

pub struct AttemptRowMetadata {
    pub response_id: Option<String>,
}

impl AttemptRow {
    /// Converts a domain Attempt to its append-only database row.
    ///
    /// # Errors
    ///
    /// Returns `internal_contract_violation` if integer counters exceed `SQLite`
    /// storage bounds or an enum cannot be serialized.
    pub fn from_domain(
        attempt: &Attempt,
        metadata: AttemptRowMetadata,
    ) -> Result<Self, PersistenceError> {
        let error = attempt.error.as_ref();

        Ok(Self {
            id: attempt.id.to_string(),
            task_id: attempt.task_id.to_string(),
            sequence: i64::from(attempt.sequence),
            api_profile_id: attempt.api_profile_id.to_string(),
            api_profile_name_snapshot: attempt.api_profile_name.clone(),
            actual_model: attempt.actual_model.clone(),
            status: encode(attempt.status)?,
            created_at: timestamp_string(&attempt.created_at),
            request_started_at: attempt.started_at.as_ref().map(timestamp_string),
            request_dispatched_at: attempt.dispatched_at.as_ref().map(timestamp_string),
            finished_at: attempt.finished_at.as_ref().map(timestamp_string),
            duration_ms: option_i64(attempt.duration_ms)?,
            http_status: attempt.http_status.map(i64::from),
            input_tokens: attempt.input_tokens.map(i64::from),
            output_tokens: attempt.output_tokens.map(i64::from),
            total_tokens: attempt.total_tokens.map(i64::from),
            retry_reason: attempt.retry_reason.clone(),
            error_code: error.map(|value| value.code.clone()),
            sanitized_error_message: error.map(|value| value.message.clone()),
            error_retryable: error.map(|value| value.retryable),
            error_sanitized: error.map(|value| value.sanitized),
            response_id: metadata.response_id,
        })
    }
}

impl TryFrom<AttemptRow> for Attempt {
    type Error = PersistenceError;

    fn try_from(row: AttemptRow) -> Result<Self, Self::Error> {
        let error = match (row.error_code, row.sanitized_error_message) {
            (None, None) => None,
            (Some(code), Some(message)) => Some(AttemptError {
                code,
                message,
                retryable: row.error_retryable.unwrap_or(false),
                sanitized: row.error_sanitized.unwrap_or(true),
            }),
            _ => return Err(PersistenceError::InvalidStoredState),
        };

        Ok(Self {
            id: AttemptId::from(row.id),
            task_id: TaskId::from(row.task_id),
            sequence: as_u32(row.sequence)?,
            api_profile_id: batch_code_analyzer_domain::ApiProfileId::from(row.api_profile_id),
            api_profile_name: row.api_profile_name_snapshot,
            actual_model: row.actual_model,
            status: decode(&row.status)?,
            created_at: Rfc3339Timestamp::new(row.created_at),
            started_at: row.request_started_at.map(Rfc3339Timestamp::new),
            dispatched_at: row.request_dispatched_at.map(Rfc3339Timestamp::new),
            finished_at: row.finished_at.map(Rfc3339Timestamp::new),
            duration_ms: option_u64(row.duration_ms)?,
            http_status: option_u16(row.http_status)?,
            input_tokens: option_u32(row.input_tokens)?,
            output_tokens: option_u32(row.output_tokens)?,
            total_tokens: option_u32(row.total_tokens)?,
            retry_reason: row.retry_reason,
            error,
        })
    }
}

fn encode<T>(value: T) -> Result<String, PersistenceError>
where
    T: Serialize,
{
    match serde_json::to_value(value).map_err(|_| PersistenceError::InvalidStoredState)? {
        Value::String(value) => Ok(value),
        value => serde_json::to_string(&value).map_err(|_| PersistenceError::InvalidStoredState),
    }
}

fn decode<T>(value: &str) -> Result<T, PersistenceError>
where
    T: DeserializeOwned,
{
    let value = serde_json::from_str(value)
        .or_else(|_| serde_json::from_value(Value::String(value.to_owned())))
        .map_err(|_| PersistenceError::InvalidStoredState)?;

    Ok(value)
}

fn as_u32(value: i64) -> Result<u32, PersistenceError> {
    u32::try_from(value).map_err(|_| PersistenceError::InvalidStoredState)
}

fn option_u32(value: Option<i64>) -> Result<Option<u32>, PersistenceError> {
    value.map(as_u32).transpose()
}

fn option_u16(value: Option<i64>) -> Result<Option<u16>, PersistenceError> {
    value
        .map(|value| u16::try_from(value).map_err(|_| PersistenceError::InvalidStoredState))
        .transpose()
}

fn option_u64(value: Option<i64>) -> Result<Option<u64>, PersistenceError> {
    value
        .map(|value| u64::try_from(value).map_err(|_| PersistenceError::InvalidStoredState))
        .transpose()
}

fn option_i64(value: Option<u64>) -> Result<Option<i64>, PersistenceError> {
    value
        .map(|value| i64::try_from(value).map_err(|_| PersistenceError::InvalidStoredState))
        .transpose()
}

fn timestamp_string(value: &Rfc3339Timestamp) -> String {
    value.as_str().to_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        AttemptRow, AttemptRowMetadata, ContextVersionRow, FileRecordRow, FileRecordRowMetadata,
        ProjectRow, ProjectRowMetadata, RunRow, RunRowMetadata, TaskRow,
    };
    use batch_code_analyzer_domain::{
        ApiProfileId, ApiRouting, Attempt, AttemptError, AttemptId, AttemptStatus, ContextStatus,
        ContextVersion, ContextVersionId, ExecutionDefaults, FileRecord, FileRecordId,
        FileResultStatus, FileSnapshot, FileSourceStatus, FilterRules, Project, ProjectContext,
        ProjectId, ProjectPathStatus, RetryPolicy, Rfc3339Timestamp, Run, RunId, RunSnapshot,
        RunStats, RunStatus, SensitiveFinding, Task, TaskId, TaskStatus, TaskValueSource,
    };

    fn timestamp() -> Rfc3339Timestamp {
        Rfc3339Timestamp::new("2026-07-17T10:00:00+08:00")
    }

    #[test]
    fn project_and_file_rows_round_trip_domain_entities() {
        let project = Project {
            schema_version: 2,
            id: ProjectId::new("project-1"),
            name: "example".into(),
            source_directory: "/workspace/example".into(),
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
                current_version_id: Some(ContextVersionId::new("context-1")),
                status: ContextStatus::Ready,
            },
            filter_rules: FilterRules::default(),
            output_root: Some("/workspace/results".into()),
            last_opened_at: timestamp(),
        };
        let project_row = ProjectRow::from_domain(
            &project,
            ProjectRowMetadata {
                canonical_source_directory: "/workspace/example".into(),
                created_at: timestamp(),
                updated_at: timestamp(),
            },
        )
        .expect("project row should serialize");
        assert_eq!(
            Project::try_from(project_row).expect("project should deserialize"),
            project
        );

        let file_record = FileRecord {
            id: FileRecordId::new("file-1"),
            project_id: ProjectId::new("project-1"),
            relative_path: "src/main.rs".into(),
            size_bytes: 123,
            modified_at: Some(timestamp()),
            content_hash: Some("blake3:file".into()),
            encoding: Some("utf-8".into()),
            language: Some("rust".into()),
            source_status: FileSourceStatus::Sensitive,
            included: false,
            exclusion_reason: Some("sensitive_content".into()),
            sensitive_findings: vec![SensitiveFinding {
                kind: "github_token".into(),
                line: Some(12),
                column: None,
            }],
            latest_successful_run_id: Some(RunId::new("run-1")),
            result_status: FileResultStatus::Stale,
        };
        let file_row = FileRecordRow::from_domain(
            &file_record,
            FileRecordRowMetadata {
                normalized_relative_path: "src/main.rs".into(),
                latest_successful_task_id: Some(TaskId::new("task-1")),
                scan_generation: 1,
                created_at: timestamp(),
                updated_at: timestamp(),
            },
        )
        .expect("file row should serialize");
        assert_eq!(
            FileRecord::try_from(file_row).expect("file record should deserialize"),
            file_record
        );
    }

    #[test]
    fn context_and_run_rows_round_trip_domain_entities() {
        let context = ContextVersion {
            id: ContextVersionId::new("context-1"),
            project_id: ProjectId::new("project-1"),
            status: ContextStatus::Ready,
            source_files: Vec::new(),
            model: Some("gpt-5".into()),
            summary: "example project".into(),
            summary_hash: "sha256:summary".into(),
            manually_edited: false,
            created_at: timestamp(),
        };
        assert_eq!(
            ContextVersion::try_from(ContextVersionRow::from(&context))
                .expect("context should deserialize"),
            context
        );

        let run = Run {
            id: RunId::new("run-1"),
            project_id: ProjectId::new("project-1"),
            status: RunStatus::Running,
            created_at: timestamp(),
            started_at: Some(timestamp()),
            completed_at: None,
            context_version_id: Some(ContextVersionId::new("context-1")),
            output_directory: "/workspace/results/run-1".into(),
            snapshot: RunSnapshot {
                api_routing: ApiRouting {
                    primary_profile_id: Some(ApiProfileId::new("profile-1")),
                    fallbacks: Vec::new(),
                },
                concurrency: 5,
                timeout_seconds: 120,
                max_output_tokens: 4096,
                retry_policy: RetryPolicy {
                    retry_count_per_profile: 1,
                },
                app_version: "0.1.0".into(),
                schema_version: 1,
            },
            stats: RunStats {
                total: 1,
                running: 1,
                ..RunStats::default()
            },
        };
        let run_row = RunRow::from_domain(
            &run,
            RunRowMetadata {
                interruption_reason: None,
            },
        )
        .expect("run row should serialize");
        assert_eq!(Run::try_from(run_row).expect("run should deserialize"), run);
    }

    #[test]
    fn task_and_attempt_rows_round_trip_domain_entities() {
        let task = Task {
            id: TaskId::new("task-1"),
            run_id: RunId::new("run-1"),
            file_id: FileRecordId::new("file-1"),
            relative_path: "src/main.rs".into(),
            file_snapshot: FileSnapshot {
                content_hash: "blake3:file".into(),
                size_bytes: 123,
            },
            prompt_snapshot: "Explain this file".into(),
            prompt_hash: "sha256:prompt".into(),
            prompt_source: TaskValueSource::Project,
            model_snapshot: "gpt-5".into(),
            model_source: TaskValueSource::Project,
            context_version_id: Some(ContextVersionId::new("context-1")),
            status: TaskStatus::Running,
            current_result_path: None,
            latest_attempt_id: Some(AttemptId::new("attempt-1")),
            parent_task_id: None,
            result_version: 1,
            created_at: timestamp(),
            started_at: Some(timestamp()),
            completed_at: None,
        };
        assert_eq!(
            Task::try_from(TaskRow::from(&task)).expect("task should deserialize"),
            task
        );

        let attempt = Attempt {
            id: AttemptId::new("attempt-1"),
            task_id: TaskId::new("task-1"),
            sequence: 1,
            api_profile_id: ApiProfileId::new("profile-1"),
            api_profile_name: "Primary".into(),
            actual_model: "gpt-5".into(),
            status: AttemptStatus::FailedRetryable,
            created_at: timestamp(),
            started_at: Some(timestamp()),
            dispatched_at: Some(timestamp()),
            finished_at: Some(timestamp()),
            duration_ms: Some(10),
            http_status: Some(429),
            input_tokens: Some(120),
            output_tokens: Some(0),
            total_tokens: Some(120),
            retry_reason: Some("rate_limited".into()),
            error: Some(AttemptError {
                code: "provider_rate_limited".into(),
                message: "request was rate limited".into(),
                retryable: true,
                sanitized: true,
            }),
        };
        let attempt_row =
            AttemptRow::from_domain(&attempt, AttemptRowMetadata { response_id: None })
                .expect("attempt row should serialize");
        assert_eq!(
            Attempt::try_from(attempt_row).expect("attempt should deserialize"),
            attempt
        );
    }
}
