//! Framework-independent entities and value objects for persisted domain data.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    ApiProfileId, AttemptId, AttemptStatus, ContextVersionId, FileRecordId, ProjectId, RunId,
    RunStatus, TaskId, TaskStatus,
};

/// A timestamp serialized as an RFC 3339 string at the application boundary.
///
/// This crate deliberately does not parse clock values. Infrastructure owns clock
/// access and request validation; the domain only preserves the stable wire form.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct Rfc3339Timestamp(String);

impl Rfc3339Timestamp {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProjectPathStatus {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FileSourceStatus {
    Normal,
    Modified,
    Deleted,
    Unreadable,
    UnsupportedEncoding,
    Sensitive,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FileResultStatus {
    None,
    Current,
    Stale,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ContextStatus {
    Ready,
    Stale,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ModelRoutingStrategy {
    UseProfileDefault,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TaskValueSource {
    Project,
    Override,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiFallback {
    pub profile_id: ApiProfileId,
    pub enabled: bool,
    pub model_strategy: ModelRoutingStrategy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiRouting {
    pub primary_profile_id: Option<ApiProfileId>,
    pub fallbacks: Vec<ApiFallback>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionDefaults {
    pub concurrency: u16,
    pub timeout_seconds: u32,
    pub max_output_tokens: u32,
    pub retry_count_per_profile: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectContext {
    pub enabled: bool,
    pub current_version_id: Option<ContextVersionId>,
    pub status: ContextStatus,
}

/// Scanner-owned settings will be added in the scanner task. Its current empty
/// representation keeps the persisted Project field stable without predefining
/// scanning behaviour in the domain foundation.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FilterRules {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub schema_version: u32,
    pub id: ProjectId,
    pub name: String,
    pub source_directory: String,
    pub path_status: ProjectPathStatus,
    pub default_prompt: String,
    pub default_model: Option<String>,
    pub context_model: Option<String>,
    pub api_routing: ApiRouting,
    pub execution_defaults: ExecutionDefaults,
    pub project_context: ProjectContext,
    pub filter_rules: FilterRules,
    pub output_root: Option<String>,
    pub last_opened_at: Rfc3339Timestamp,
}

/// A sensitive finding contains classification and location metadata only.
/// The matched secret value is never part of the domain model.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveFinding {
    pub kind: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub id: FileRecordId,
    pub project_id: ProjectId,
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_at: Option<Rfc3339Timestamp>,
    pub content_hash: Option<String>,
    pub encoding: Option<String>,
    pub language: Option<String>,
    pub source_status: FileSourceStatus,
    pub included: bool,
    pub exclusion_reason: Option<String>,
    pub sensitive_findings: Vec<SensitiveFinding>,
    pub latest_successful_run_id: Option<RunId>,
    pub result_status: FileResultStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RetryPolicy {
    pub retry_count_per_profile: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunSnapshot {
    pub api_routing: ApiRouting,
    pub concurrency: u16,
    pub timeout_seconds: u32,
    pub max_output_tokens: u32,
    pub retry_policy: RetryPolicy,
    pub app_version: String,
    pub schema_version: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunStats {
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: RunId,
    pub project_id: ProjectId,
    pub status: RunStatus,
    pub created_at: Rfc3339Timestamp,
    pub started_at: Option<Rfc3339Timestamp>,
    pub completed_at: Option<Rfc3339Timestamp>,
    pub context_version_id: Option<ContextVersionId>,
    pub output_directory: String,
    pub snapshot: RunSnapshot,
    pub stats: RunStats,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileSnapshot {
    pub content_hash: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: TaskId,
    pub run_id: RunId,
    pub file_id: FileRecordId,
    pub relative_path: String,
    pub file_snapshot: FileSnapshot,
    pub prompt_snapshot: String,
    pub prompt_hash: String,
    pub prompt_source: TaskValueSource,
    pub model_snapshot: String,
    pub model_source: TaskValueSource,
    pub context_version_id: Option<ContextVersionId>,
    pub status: TaskStatus,
    pub current_result_path: Option<String>,
    pub latest_attempt_id: Option<AttemptId>,
    pub parent_task_id: Option<TaskId>,
    pub result_version: u32,
    pub created_at: Rfc3339Timestamp,
    pub started_at: Option<Rfc3339Timestamp>,
    pub completed_at: Option<Rfc3339Timestamp>,
}

/// Error information retained for an Attempt after sanitization.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AttemptError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub sanitized: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Attempt {
    pub id: AttemptId,
    pub task_id: TaskId,
    pub sequence: u32,
    pub api_profile_id: ApiProfileId,
    pub api_profile_name: String,
    pub actual_model: String,
    pub status: AttemptStatus,
    pub created_at: Rfc3339Timestamp,
    pub started_at: Option<Rfc3339Timestamp>,
    pub dispatched_at: Option<Rfc3339Timestamp>,
    pub finished_at: Option<Rfc3339Timestamp>,
    pub duration_ms: Option<u64>,
    pub http_status: Option<u16>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub retry_reason: Option<String>,
    pub error: Option<AttemptError>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContextVersionSourceFile {
    pub relative_path: String,
    pub content_hash: String,
    pub included: bool,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContextVersion {
    pub id: ContextVersionId,
    pub project_id: ProjectId,
    pub status: ContextStatus,
    pub source_files: Vec<ContextVersionSourceFile>,
    pub model: Option<String>,
    pub summary: String,
    pub summary_hash: String,
    pub manually_edited: bool,
    pub created_at: Rfc3339Timestamp,
}

#[cfg(test)]
mod tests {
    use serde_json::{json, to_value};

    use super::{
        ApiRouting, Attempt, AttemptError, ContextStatus, ContextVersion, ExecutionDefaults,
        FileRecord, FileResultStatus, FileSourceStatus, FilterRules, Project, ProjectContext,
        ProjectPathStatus, Rfc3339Timestamp, Run, RunSnapshot, RunStats, Task, TaskValueSource,
    };
    use crate::{
        ApiProfileId, AttemptId, AttemptStatus, ContextVersionId, FileRecordId, ProjectId, RunId,
        RunStatus, TaskId, TaskStatus,
    };

    fn timestamp() -> Rfc3339Timestamp {
        Rfc3339Timestamp::new("2026-07-15T10:38:25+08:00")
    }

    #[test]
    fn project_serializes_with_documented_stable_field_and_enum_names() {
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

        assert_eq!(
            to_value(project).expect("project should serialize"),
            json!({
                "schemaVersion": 2,
                "id": "project-1",
                "name": "example",
                "sourceDirectory": "/workspace/example",
                "pathStatus": "available",
                "defaultPrompt": "Explain this file",
                "defaultModel": "gpt-5",
                "contextModel": null,
                "apiRouting": { "primaryProfileId": "profile-1", "fallbacks": [] },
                "executionDefaults": {
                    "concurrency": 5,
                    "timeoutSeconds": 120,
                    "maxOutputTokens": 4096,
                    "retryCountPerProfile": 1
                },
                "projectContext": {
                    "enabled": true,
                    "currentVersionId": "context-1",
                    "status": "ready"
                },
                "filterRules": {},
                "outputRoot": "/workspace/results",
                "lastOpenedAt": "2026-07-15T10:38:25+08:00"
            })
        );
    }

    #[test]
    fn project_serializes_without_an_api_profile_before_configuration() {
        let routing = ApiRouting {
            primary_profile_id: None,
            fallbacks: Vec::new(),
        };

        let value = to_value(routing).expect("empty routing should serialize");
        assert_eq!(value["primaryProfileId"], serde_json::Value::Null);
        assert_eq!(
            serde_json::from_value::<ApiRouting>(value).expect("routing should deserialize"),
            ApiRouting {
                primary_profile_id: None,
                fallbacks: Vec::new(),
            }
        );
    }

    #[test]
    fn project_deserializes_legacy_configured_api_routing() {
        let legacy = serde_json::json!({
            "primaryProfileId": "profile-1",
            "fallbacks": [],
        });

        assert_eq!(
            serde_json::from_value::<ApiRouting>(legacy).expect("legacy routing should decode"),
            ApiRouting {
                primary_profile_id: Some(ApiProfileId::new("profile-1")),
                fallbacks: Vec::new(),
            }
        );
    }

    #[test]
    fn file_record_serializes_optional_scan_metadata_without_secret_values() {
        let file_record = FileRecord {
            id: FileRecordId::new("file-1"),
            project_id: ProjectId::new("project-1"),
            relative_path: "src/main.rs".into(),
            size_bytes: 123,
            modified_at: Some(timestamp()),
            content_hash: Some("blake3:example".into()),
            encoding: Some("utf-8".into()),
            language: Some("rust".into()),
            source_status: FileSourceStatus::Sensitive,
            included: false,
            exclusion_reason: Some("sensitive_content".into()),
            sensitive_findings: vec![super::SensitiveFinding {
                kind: "github_token".into(),
                line: Some(12),
                column: None,
            }],
            latest_successful_run_id: None,
            result_status: FileResultStatus::None,
        };

        let value = to_value(file_record).expect("file record should serialize");
        assert_eq!(value["sourceStatus"], "sensitive");
        assert_eq!(value["sensitiveFindings"][0]["kind"], "github_token");
        assert!(value.to_string().contains("github_token"));
        assert!(!value.to_string().contains("ghp_"));
    }

    #[test]
    fn run_task_attempt_and_context_version_preserve_snapshot_relationships() {
        let run = Run {
            id: RunId::new("run-1"),
            project_id: ProjectId::new("project-1"),
            status: RunStatus::Running,
            created_at: timestamp(),
            started_at: Some(timestamp()),
            completed_at: None,
            context_version_id: Some(ContextVersionId::new("context-1")),
            output_directory: "/workspace/results/runs/run-1".into(),
            snapshot: RunSnapshot {
                api_routing: ApiRouting {
                    primary_profile_id: Some(ApiProfileId::new("profile-1")),
                    fallbacks: Vec::new(),
                },
                concurrency: 5,
                timeout_seconds: 120,
                max_output_tokens: 4096,
                retry_policy: super::RetryPolicy {
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
        let task = Task {
            id: TaskId::new("task-1"),
            run_id: RunId::new("run-1"),
            file_id: FileRecordId::new("file-1"),
            relative_path: "src/main.rs".into(),
            file_snapshot: super::FileSnapshot {
                content_hash: "blake3:example".into(),
                size_bytes: 123,
            },
            prompt_snapshot: "Explain this file".into(),
            prompt_hash: "sha256:prompt".into(),
            prompt_source: TaskValueSource::Project,
            model_snapshot: "gpt-5".into(),
            model_source: TaskValueSource::Override,
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
        let context_version = ContextVersion {
            id: ContextVersionId::new("context-1"),
            project_id: ProjectId::new("project-1"),
            status: ContextStatus::Ready,
            source_files: Vec::new(),
            model: Some("gpt-5".into()),
            summary: "Example project".into(),
            summary_hash: "sha256:summary".into(),
            manually_edited: false,
            created_at: timestamp(),
        };

        let run_json = to_value(run).expect("run should serialize");
        let task_json = to_value(task).expect("task should serialize");
        let attempt_json = to_value(attempt).expect("attempt should serialize");
        let context_json = to_value(context_version).expect("context should serialize");

        assert_eq!(run_json["snapshot"]["schemaVersion"], 1);
        assert_eq!(task_json["promptSource"], "project");
        assert_eq!(task_json["modelSource"], "override");
        assert_eq!(attempt_json["status"], "failed_retryable");
        assert_eq!(attempt_json["error"]["sanitized"], true);
        assert_eq!(context_json["status"], "ready");
    }
}
