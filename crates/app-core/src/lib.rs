//! Application-layer orchestration for Batch Code Analyzer.

#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    hash::{Hash, Hasher},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use batch_code_analyzer_api_profiles::ApiProfile as ProviderApiProfile;
use batch_code_analyzer_domain::{
    ApiModelInfo, ApiProfile, ApiProfileConnectionStatus, ApiProfileId, ApiProtocol, ApiRouting,
    Attempt, AttemptError, AttemptId, AttemptStatus, ContextStatus, ContextVersion,
    ContextVersionId, ContextVersionSourceFile, ExecutionDefaults, FileRecord, FileRecordId,
    FileResultStatus, FileSnapshot, FileSourceStatus, FilterRules, Project, ProjectContext,
    ProjectId, ProjectPathStatus, RetryPolicy, Rfc3339Timestamp, Run, RunId, RunSnapshot,
    RunStateMachine, RunStatus, RunTransition, SensitiveFinding, Task, TaskId, TaskStateMachine,
    TaskStatus, TaskTransition, TaskValueSource,
};
use batch_code_analyzer_model_providers::{
    ModelProvider, OpenAiResponsesProvider, ProviderRequest,
};
use batch_code_analyzer_persistence::{
    AttemptRowMetadata, Database, PersistenceError, ProjectRowMetadata,
    SensitiveFileAuthorizationMetadata, LATEST_SCHEMA_VERSION,
};
use batch_code_analyzer_repository_scanner::{
    FileDecision, ImportReport, ScanCancellation, ScanConfig, ScanError, ScanResult, Scanner,
    DEFAULT_MAX_FILE_SIZE,
};
use batch_code_analyzer_secret_store::SecretRef;
use batch_code_analyzer_security_core::{content_hash, detect_secrets, SafeRoot};
use serde::Serialize;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio_util::sync::CancellationToken;

pub use batch_code_analyzer_domain as domain;

const DEFAULT_PROMPT: &str = "请结合提供的项目上下文，用通俗但准确的语言解释当前代码文件。\n请说明：\n1. 该文件在项目中的核心职责；\n2. 关键输入、输出、状态或数据流；\n3. 它与哪些模块或功能协作，以及它为何存在；\n4. 修改或缺失它可能带来的影响。\n如无法从上下文或代码中确认，请明确说明不确定性，不要臆测。";
static NEXT_PROJECT_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_API_PROFILE_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_ATTEMPT_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_CONTEXT_VERSION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Eq, PartialEq)]
pub enum ProjectServiceError {
    NotFound,
    ApiProfileNotFound,
    PathUnavailable,
    Persistence(PersistenceError),
}

impl ProjectServiceError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "project_not_found",
            Self::ApiProfileNotFound => "validation_invalid_value",
            Self::PathUnavailable => "project_path_unavailable",
            Self::Persistence(error) => error.code(),
        }
    }
}

impl std::fmt::Display for ProjectServiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProjectServiceError {}

impl From<PersistenceError> for ProjectServiceError {
    fn from(error: PersistenceError) -> Self {
        Self::Persistence(error)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum FileServiceError {
    NotFound,
    SensitiveConfirmationRequired,
    SensitiveBlocked,
    Unreadable,
    UnsupportedEncoding,
    Binary,
    TooLarge,
    RuleExcluded,
    Deleted,
    Persistence(PersistenceError),
}

impl FileServiceError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotFound | Self::Deleted | Self::RuleExcluded => "validation_invalid_value",
            Self::SensitiveConfirmationRequired => "security_sensitive_confirmation_required",
            Self::SensitiveBlocked => "security_sensitive_file_blocked",
            Self::Unreadable => "scan_file_unreadable",
            Self::UnsupportedEncoding => "scan_encoding_unsupported",
            Self::Binary => "scan_binary_file",
            Self::TooLarge => "scan_file_too_large",
            Self::Persistence(error) => error.code(),
        }
    }
}

impl std::fmt::Display for FileServiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for FileServiceError {}

impl From<PersistenceError> for FileServiceError {
    fn from(error: PersistenceError) -> Self {
        Self::Persistence(error)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ApiProfileServiceError {
    NotFound,
    InvalidName,
    InvalidBaseUrl,
    UrlContainsCredentials,
    Persistence(PersistenceError),
}

impl ApiProfileServiceError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotFound | Self::InvalidName | Self::InvalidBaseUrl => "validation_invalid_value",
            Self::UrlContainsCredentials => "security_invalid_secret_reference",
            Self::Persistence(error) => error.code(),
        }
    }
}

impl std::fmt::Display for ApiProfileServiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ApiProfileServiceError {}

impl From<PersistenceError> for ApiProfileServiceError {
    fn from(error: PersistenceError) -> Self {
        Self::Persistence(error)
    }
}

#[derive(Debug)]
pub struct ProjectAddResult {
    pub project: Project,
    pub created: bool,
    pub config_mirror_warning: bool,
}

#[derive(Debug)]
pub struct ProjectRunSettingsResult {
    pub project: Project,
    pub config_mirror_warning: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RunPreparationInput {
    pub prompt: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunPreview {
    pub project_id: ProjectId,
    pub tasks: Vec<RunPreviewTask>,
    pub blockers: Vec<RunBlockingReason>,
    pub model: Option<String>,
    pub prompt_source: TaskValueSource,
    pub model_source: TaskValueSource,
    pub output_directory: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunPreviewTask {
    pub file_id: FileRecordId,
    pub relative_path: String,
    pub size_bytes: u64,
    pub content_hash: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunBlockingReason {
    pub code: &'static str,
    pub message: &'static str,
    pub file_id: Option<FileRecordId>,
    pub relative_path: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RunServiceError {
    NotFound,
    ActiveRun,
    Blocked(RunBlockingReason),
    Persistence(PersistenceError),
}

impl RunServiceError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "project_not_found",
            Self::ActiveRun => "run_active_exists",
            Self::Blocked(reason) => reason.code,
            Self::Persistence(error) => error.code(),
        }
    }
}

impl std::fmt::Display for RunServiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RunServiceError {}

impl From<PersistenceError> for RunServiceError {
    fn from(error: PersistenceError) -> Self {
        Self::Persistence(error)
    }
}

pub struct RunService<'database> {
    database: &'database Database,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RunExecutionError {
    NotFound,
    NotRunning,
    PathUnavailable,
    OutputWriteFailed,
    Persistence(PersistenceError),
}

impl RunExecutionError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "run_not_found",
            Self::NotRunning => "run_not_active",
            Self::PathUnavailable => "project_path_unavailable",
            Self::OutputWriteFailed => "output_write_failed",
            Self::Persistence(error) => error.code(),
        }
    }
}

impl std::fmt::Display for RunExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RunExecutionError {}

impl From<PersistenceError> for RunExecutionError {
    fn from(error: PersistenceError) -> Self {
        Self::Persistence(error)
    }
}

pub struct RunExecutionService<'database> {
    database: &'database Database,
    provider: OpenAiResponsesProvider,
}

pub struct ProjectService<'database> {
    database: &'database Database,
}

pub struct ContextService<'database> {
    database: &'database Database,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ContextServiceError {
    NotFound,
    PathUnavailable,
    DiscoveryFailed,
    Persistence(PersistenceError),
}

impl ContextServiceError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "project_not_found",
            Self::PathUnavailable => "project_path_unavailable",
            Self::DiscoveryFailed => "context_discovery_failed",
            Self::Persistence(error) => error.code(),
        }
    }
}

impl std::fmt::Display for ContextServiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ContextServiceError {}

impl From<PersistenceError> for ContextServiceError {
    fn from(error: PersistenceError) -> Self {
        Self::Persistence(error)
    }
}

pub struct ApiProfileService<'database> {
    database: &'database Database,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ScanServiceError {
    PathUnavailable,
    Cancelled,
    ScanFailed,
    Persistence(PersistenceError),
}

impl ScanServiceError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::PathUnavailable => "project_path_unavailable",
            Self::Cancelled => "scan_cancelled",
            Self::ScanFailed => "scan_failed",
            Self::Persistence(error) => error.code(),
        }
    }
}

impl std::fmt::Display for ScanServiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ScanServiceError {}

impl From<PersistenceError> for ScanServiceError {
    fn from(error: PersistenceError) -> Self {
        Self::Persistence(error)
    }
}

#[derive(Clone, Debug)]
pub struct ScanSummary {
    pub generation: u32,
    pub file_count: u32,
    pub report: ImportReport,
}

pub struct ScanService<'database> {
    database: &'database Database,
}

impl<'database> ScanService<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }

    /// Runs the scanner synchronously. Callers must execute this method on a
    /// blocking thread and only call `persist_scan` after `completed` is true.
    ///
    /// # Errors
    ///
    /// Returns a stable scan error without exposing scanner paths or driver
    /// diagnostics.
    pub fn scan_project(
        project: &Project,
        cancellation: ScanCancellation,
    ) -> Result<ScanResult, ScanServiceError> {
        Self::scan_project_with_patterns(project, cancellation, Vec::new())
    }

    /// Runs the scanner with temporary user patterns that apply only to this
    /// scan session.
    ///
    /// # Errors
    ///
    /// Returns a stable scan error without exposing scanner paths or driver
    /// diagnostics.
    pub fn scan_project_with_patterns(
        project: &Project,
        cancellation: ScanCancellation,
        temporary_excluded_patterns: Vec<String>,
    ) -> Result<ScanResult, ScanServiceError> {
        Scanner::new(ScanConfig {
            root: project.source_directory.clone().into(),
            cancellation,
            excluded_patterns: temporary_excluded_patterns,
            ..ScanConfig::new(project.source_directory.clone())
        })
        .scan()
        .map_err(|error| match error {
            ScanError::Root("project_path_unavailable") => ScanServiceError::PathUnavailable,
            ScanError::Root(_) | ScanError::Io(_) => ScanServiceError::ScanFailed,
        })
    }

    /// Converts a completed scanner result to Domain rows and commits one
    /// generation atomically.
    ///
    /// # Errors
    ///
    /// Returns a stable persistence error when the generation cannot commit.
    pub async fn persist_scan(
        &self,
        project_id: &ProjectId,
        result: ScanResult,
    ) -> Result<ScanSummary, ScanServiceError> {
        if !result.completed {
            return Err(ScanServiceError::Cancelled);
        }
        let existing = self
            .database
            .repository()
            .list_file_records(project_id)
            .await?;
        let records = map_scanned_files(project_id, &existing, &result);
        let now = timestamp_now();
        let generation = self
            .database
            .repository()
            .commit_scan(project_id, &records, &now)
            .await?;
        Ok(ScanSummary {
            generation,
            file_count: u32::try_from(records.len()).unwrap_or(u32::MAX),
            report: result.report,
        })
    }
}

#[allow(clippy::too_many_lines)]
fn map_scanned_files(
    project_id: &ProjectId,
    existing: &[FileRecord],
    result: &ScanResult,
) -> Vec<FileRecord> {
    let existing_by_path: BTreeMap<&str, &FileRecord> = existing
        .iter()
        .map(|record| (record.relative_path.as_str(), record))
        .collect();
    result
        .files
        .iter()
        .map(|scanned| {
            let previous = existing_by_path
                .get(scanned.relative_path.as_str())
                .copied();
            let (source_status, included, exclusion_reason) = match &scanned.decision {
                FileDecision::Included => {
                    let status = if previous.is_some_and(|record| {
                        record.content_hash.is_some() && record.content_hash != scanned.content_hash
                    }) {
                        FileSourceStatus::Modified
                    } else {
                        FileSourceStatus::Normal
                    };
                    if previous.is_some_and(|record| {
                        record.exclusion_reason.as_deref() == Some("user_excluded")
                    }) {
                        (status, false, Some("user_excluded".into()))
                    } else {
                        (status, true, None)
                    }
                }
                FileDecision::Excluded { reason } => {
                    (FileSourceStatus::Normal, false, Some(reason.clone()))
                }
                FileDecision::Unreadable => (
                    FileSourceStatus::Unreadable,
                    false,
                    Some("unreadable".into()),
                ),
                FileDecision::Binary => (FileSourceStatus::Normal, false, Some("binary".into())),
                FileDecision::UnsupportedEncoding => (
                    FileSourceStatus::UnsupportedEncoding,
                    false,
                    Some("unsupported_encoding".into()),
                ),
                FileDecision::Sensitive => {
                    (FileSourceStatus::Sensitive, false, Some("sensitive".into()))
                }
                FileDecision::TooLarge => (
                    FileSourceStatus::Normal,
                    false,
                    Some("file_too_large".into()),
                ),
                FileDecision::Symlink => (FileSourceStatus::Normal, false, Some("symlink".into())),
            };
            let changed = previous.is_some_and(|record| {
                record.content_hash != scanned.content_hash || record.source_status != source_status
            });
            let result_status = match previous {
                Some(record) if changed && record.result_status != FileResultStatus::None => {
                    FileResultStatus::Stale
                }
                Some(record) => record.result_status,
                None => FileResultStatus::None,
            };
            FileRecord {
                id: previous.map_or_else(new_file_id, |record| record.id.clone()),
                project_id: project_id.clone(),
                relative_path: scanned.relative_path.clone(),
                size_bytes: scanned.size_bytes,
                modified_at: scanned
                    .modified_at
                    .as_ref()
                    .map(|value| Rfc3339Timestamp::new(value.clone())),
                content_hash: scanned.content_hash.clone(),
                encoding: scanned.encoding.clone(),
                language: scanned.language.clone(),
                source_status,
                included,
                exclusion_reason,
                sensitive_findings: scanned
                    .sensitive_findings
                    .iter()
                    .map(|finding| SensitiveFinding {
                        kind: finding.kind.clone(),
                        line: Some(finding.line),
                        column: Some(finding.column),
                    })
                    .collect(),
                latest_successful_run_id: previous
                    .and_then(|record| record.latest_successful_run_id.clone()),
                result_status,
            }
        })
        .collect()
}

impl<'database> ProjectService<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }

    /// Registers an existing directory or returns the already registered project.
    ///
    /// `SQLite` is committed before the optional `.batch-analysis` mirror is
    /// written. A mirror failure is reported as a warning and never rolls back
    /// a successful project registration.
    ///
    /// # Errors
    ///
    /// Returns `project_path_unavailable` for a missing/non-directory path or a
    /// stable persistence error when the project cannot be stored.
    pub async fn add_project(
        &self,
        source_directory: impl AsRef<Path>,
    ) -> Result<ProjectAddResult, ProjectServiceError> {
        let root =
            SafeRoot::new(source_directory).map_err(|_| ProjectServiceError::PathUnavailable)?;
        let canonical = root.path().to_path_buf();
        let canonical_string = canonical.to_string_lossy().into_owned();
        if let Some(project) = self
            .database
            .repository()
            .find_project_by_canonical_path(&canonical_string)
            .await?
        {
            return Ok(ProjectAddResult {
                project,
                created: false,
                config_mirror_warning: false,
            });
        }

        let now = timestamp_now();
        let project = Project {
            schema_version: LATEST_SCHEMA_VERSION,
            id: new_project_id(),
            name: display_name(&canonical),
            source_directory: canonical_string.clone(),
            path_status: ProjectPathStatus::Available,
            default_prompt: DEFAULT_PROMPT.into(),
            default_model: None,
            context_model: None,
            api_routing: ApiRouting {
                primary_profile_id: None,
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
            output_root: None,
            last_opened_at: now.clone(),
        };
        let create_result = self
            .database
            .repository()
            .create_project(
                &project,
                ProjectRowMetadata {
                    canonical_source_directory: canonical_string.clone(),
                    created_at: now.clone(),
                    updated_at: now,
                },
            )
            .await;
        if let Err(error) = create_result {
            if matches!(
                error,
                PersistenceError::StateTransition {
                    code: "project_path_duplicate"
                }
            ) {
                if let Some(existing) = self
                    .database
                    .repository()
                    .find_project_by_canonical_path(&canonical_string)
                    .await?
                {
                    return Ok(ProjectAddResult {
                        project: existing,
                        created: false,
                        config_mirror_warning: false,
                    });
                }
            }
            return Err(ProjectServiceError::Persistence(error));
        }

        let config_mirror_warning = write_project_mirror(&project).is_err();
        Ok(ProjectAddResult {
            project,
            created: true,
            config_mirror_warning,
        })
    }

    /// Updates the API routing and default model used by future Runs.
    ///
    /// `SQLite` remains authoritative. The portable project mirror is written
    /// only after the database transaction commits and never contains secrets.
    ///
    /// # Errors
    ///
    /// Returns a stable not-found error for a missing Project or API Profile,
    /// or a persistence error when the settings cannot be committed.
    pub async fn update_run_settings(
        &self,
        project_id: &ProjectId,
        primary_profile_id: Option<ApiProfileId>,
        default_model: Option<String>,
    ) -> Result<ProjectRunSettingsResult, ProjectServiceError> {
        let mut project = self
            .database
            .repository()
            .get_project(project_id)
            .await?
            .ok_or(ProjectServiceError::NotFound)?;
        if let Some(profile_id) = primary_profile_id.as_ref() {
            let profile_exists = self
                .database
                .repository()
                .get_api_profile(profile_id)
                .await?
                .is_some();
            if !profile_exists {
                return Err(ProjectServiceError::ApiProfileNotFound);
            }
        }
        project.api_routing.primary_profile_id = primary_profile_id;
        project.default_model = default_model
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        self.database
            .repository()
            .update_project(
                &project,
                ProjectRowMetadata {
                    canonical_source_directory: project.source_directory.clone(),
                    created_at: project.last_opened_at.clone(),
                    updated_at: timestamp_now(),
                },
            )
            .await?;
        let config_mirror_warning = write_project_mirror(&project).is_err();
        Ok(ProjectRunSettingsResult {
            project,
            config_mirror_warning,
        })
    }

    /// Lists project summaries from the `SQLite` source of truth.
    ///
    /// # Errors
    ///
    /// Returns a stable persistence error when the list cannot be read.
    pub async fn list_projects(&self) -> Result<Vec<Project>, PersistenceError> {
        self.database.repository().list_projects().await
    }

    /// Lists a project's persisted file records for the file table.
    ///
    /// # Errors
    ///
    /// Returns a stable persistence error when records cannot be read.
    pub async fn list_file_records(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<FileRecord>, PersistenceError> {
        self.database
            .repository()
            .list_file_records(project_id)
            .await
    }

    /// Applies a user's inclusion choice without changing scanner facts.
    ///
    /// Security and readability exclusions remain blocked until a dedicated
    /// consent or recovery flow exists.
    ///
    /// # Errors
    ///
    /// Returns a stable file error when the record is missing or cannot be
    /// safely included, and a persistence error when the update fails.
    pub async fn set_file_included(
        &self,
        project_id: &ProjectId,
        file_id: &FileRecordId,
        included: bool,
    ) -> Result<FileRecord, FileServiceError> {
        let file = self
            .database
            .repository()
            .get_file_record(file_id)
            .await?
            .filter(|record| record.project_id == *project_id)
            .ok_or(FileServiceError::NotFound)?;

        if included {
            match file.source_status {
                FileSourceStatus::Sensitive => return Err(FileServiceError::SensitiveBlocked),
                FileSourceStatus::Unreadable => return Err(FileServiceError::Unreadable),
                FileSourceStatus::UnsupportedEncoding => {
                    return Err(FileServiceError::UnsupportedEncoding);
                }
                FileSourceStatus::Deleted => return Err(FileServiceError::Deleted),
                FileSourceStatus::Normal | FileSourceStatus::Modified => {}
            }
            match file.exclusion_reason.as_deref() {
                Some("binary") => return Err(FileServiceError::Binary),
                Some("file_too_large") => return Err(FileServiceError::TooLarge),
                Some("builtin_extension" | "symlink") => {
                    return Err(FileServiceError::RuleExcluded);
                }
                _ => {}
            }
        }

        self.database
            .repository()
            .set_file_included(project_id, file_id, included, &timestamp_now())
            .await?
            .ok_or(FileServiceError::NotFound)
    }

    /// Revalidates a sensitive file and records explicit user authorization to
    /// include its current contents in a future Run.
    ///
    /// The file remains marked as sensitive. Only a safe hash, encoding, size,
    /// timestamp, and redacted finding metadata are persisted; source content
    /// never crosses the application boundary.
    ///
    /// # Errors
    ///
    /// Returns a stable security or file error when confirmation, path safety,
    /// readability, encoding, binary, or size validation fails.
    pub async fn authorize_sensitive_file(
        &self,
        project_id: &ProjectId,
        file_id: &FileRecordId,
        confirmed: bool,
    ) -> Result<FileRecord, FileServiceError> {
        if !confirmed {
            return Err(FileServiceError::SensitiveConfirmationRequired);
        }
        let file = self
            .database
            .repository()
            .get_file_record(file_id)
            .await?
            .filter(|record| record.project_id == *project_id)
            .ok_or(FileServiceError::NotFound)?;
        if file.source_status != FileSourceStatus::Sensitive {
            return Err(FileServiceError::RuleExcluded);
        }
        let project = self
            .database
            .repository()
            .get_project(project_id)
            .await?
            .ok_or(FileServiceError::NotFound)?;
        let root =
            SafeRoot::new(&project.source_directory).map_err(|_| FileServiceError::Unreadable)?;
        let relative = root
            .relative_path(&file.relative_path)
            .map_err(|_| FileServiceError::RuleExcluded)?;
        let path = root.path().join(relative);
        let metadata = fs::symlink_metadata(&path).map_err(|_| FileServiceError::Unreadable)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(FileServiceError::RuleExcluded);
        }
        if metadata.len() > DEFAULT_MAX_FILE_SIZE {
            return Err(FileServiceError::TooLarge);
        }
        let bytes = fs::read(&path).map_err(|_| FileServiceError::Unreadable)?;
        if bytes.contains(&0) {
            return Err(FileServiceError::Binary);
        }
        let content =
            std::str::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes))
                .map_err(|_| FileServiceError::UnsupportedEncoding)?;
        let sensitive_findings = detect_secrets(content)
            .into_iter()
            .map(|finding| SensitiveFinding {
                kind: finding.kind,
                line: Some(finding.line),
                column: Some(finding.column),
            })
            .collect::<Vec<_>>();
        let modified_at = metadata.modified().ok().and_then(|time| {
            time.duration_since(UNIX_EPOCH)
                .ok()
                .map(|duration| Rfc3339Timestamp::new(format!("unix:{}", duration.as_secs())))
        });
        self.database
            .repository()
            .authorize_sensitive_file(
                project_id,
                file_id,
                SensitiveFileAuthorizationMetadata {
                    size_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                    modified_at,
                    content_hash: content_hash(&bytes),
                    encoding: "utf-8".into(),
                    sensitive_findings,
                    updated_at: timestamp_now(),
                },
            )
            .await?
            .ok_or(FileServiceError::NotFound)
    }

    /// Loads one project with its path for the detail view.
    ///
    /// # Errors
    ///
    /// Returns a stable persistence error when the project cannot be read.
    pub async fn get_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<Project>, PersistenceError> {
        self.database.repository().get_project(project_id).await
    }
}

impl<'database> RunService<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }

    /// Builds a read-only preview of the next immutable Run snapshot.
    ///
    /// # Errors
    ///
    /// Returns a stable project or persistence error when the project cannot
    /// be loaded. Validation blockers are returned in the preview itself.
    pub async fn preview(
        &self,
        project_id: &ProjectId,
        input: &RunPreparationInput,
    ) -> Result<RunPreview, RunServiceError> {
        self.prepare(project_id, input).await
    }

    /// Creates a running Run and queued Tasks from one validated snapshot.
    ///
    /// # Errors
    ///
    /// Returns `run_active_exists` when another Run is active, a validation
    /// blocker when the snapshot is incomplete, or a persistence error when
    /// the transaction cannot commit.
    #[allow(clippy::too_many_lines)]
    pub async fn create(
        &self,
        project_id: &ProjectId,
        input: &RunPreparationInput,
    ) -> Result<Run, RunServiceError> {
        let prepared = self.prepare(project_id, input).await?;
        if prepared
            .blockers
            .iter()
            .any(|blocker| blocker.code == "run_active_exists")
        {
            return Err(RunServiceError::ActiveRun);
        }
        if let Some(blocker) = prepared.blockers.first().cloned() {
            return Err(RunServiceError::Blocked(blocker));
        }

        let now = timestamp_now();
        let run_id = new_run_id();
        let run_status = RunStateMachine::transition(RunStatus::Draft, RunTransition::Start)
            .map_err(|_| RunServiceError::Blocked(invalid_run_blocker()))?;
        let task_status =
            TaskStateMachine::transition(TaskStatus::Pending, TaskTransition::Enqueue)
                .map_err(|_| RunServiceError::Blocked(invalid_task_blocker()))?;
        let project = self
            .database
            .repository()
            .get_project(project_id)
            .await?
            .ok_or(RunServiceError::NotFound)?;
        let output_directory = Path::new(&prepared.output_directory)
            .join(run_id.as_str())
            .to_string_lossy()
            .into_owned();
        let run = Run {
            id: run_id.clone(),
            project_id: project_id.clone(),
            status: run_status,
            created_at: now.clone(),
            started_at: Some(now.clone()),
            completed_at: None,
            context_version_id: project.project_context.current_version_id.clone(),
            output_directory,
            snapshot: RunSnapshot {
                api_routing: project.api_routing.clone(),
                concurrency: project.execution_defaults.concurrency,
                timeout_seconds: project.execution_defaults.timeout_seconds,
                max_output_tokens: project.execution_defaults.max_output_tokens,
                retry_policy: RetryPolicy {
                    retry_count_per_profile: project.execution_defaults.retry_count_per_profile,
                },
                app_version: env!("CARGO_PKG_VERSION").to_owned(),
                schema_version: LATEST_SCHEMA_VERSION,
            },
            stats: batch_code_analyzer_domain::RunStats::default(),
        };
        let prompt = resolve_prompt(input, &project);
        let prompt_source = if input.prompt.is_some() {
            TaskValueSource::Override
        } else {
            TaskValueSource::Project
        };
        let model = prepared
            .model
            .clone()
            .ok_or_else(|| RunServiceError::Blocked(model_missing_blocker()))?;
        let model_source = if input.model.is_some() {
            TaskValueSource::Override
        } else {
            TaskValueSource::Project
        };
        let tasks = prepared
            .tasks
            .iter()
            .map(|file| Task {
                id: new_task_id(),
                run_id: run_id.clone(),
                file_id: file.file_id.clone(),
                relative_path: file.relative_path.clone(),
                file_snapshot: FileSnapshot {
                    content_hash: file.content_hash.clone().unwrap_or_default(),
                    size_bytes: file.size_bytes,
                },
                prompt_snapshot: prompt.clone(),
                prompt_hash: hash_prompt(&prompt),
                prompt_source,
                model_snapshot: model.clone(),
                model_source,
                context_version_id: run.context_version_id.clone(),
                status: task_status,
                current_result_path: None,
                latest_attempt_id: None,
                parent_task_id: None,
                result_version: 1,
                created_at: now.clone(),
                started_at: None,
                completed_at: None,
            })
            .collect::<Vec<_>>();
        self.database
            .repository()
            .create_run_with_tasks(
                &run,
                batch_code_analyzer_persistence::RunRowMetadata {
                    interruption_reason: None,
                },
                &tasks,
            )
            .await
            .map_err(Into::into)
    }

    #[allow(clippy::too_many_lines)]
    async fn prepare(
        &self,
        project_id: &ProjectId,
        input: &RunPreparationInput,
    ) -> Result<RunPreview, RunServiceError> {
        let project = self
            .database
            .repository()
            .get_project(project_id)
            .await?
            .ok_or(RunServiceError::NotFound)?;
        let mut blockers = Vec::new();
        if project.path_status != ProjectPathStatus::Available {
            blockers.push(path_unavailable_blocker());
        }
        let active_runs = self.database.repository().unfinished_runs().await?;
        if !active_runs.is_empty() {
            blockers.push(active_run_blocker());
        }

        let profile = match project.api_routing.primary_profile_id.as_ref() {
            Some(profile_id) => {
                self.database
                    .repository()
                    .get_api_profile(profile_id)
                    .await?
            }
            None => None,
        };
        if project.api_routing.primary_profile_id.is_none() || profile.is_none() {
            blockers.push(api_profile_missing_blocker());
        } else if profile
            .as_ref()
            .is_some_and(|value| value.secret_ref.is_none())
        {
            blockers.push(secret_missing_blocker());
        }

        let model = input
            .model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                profile
                    .as_ref()
                    .and_then(|value| value.default_model.clone())
            })
            .or_else(|| project.default_model.clone());
        if model.is_none() {
            blockers.push(model_missing_blocker());
        }
        let prompt = resolve_prompt(input, &project);
        if prompt.trim().is_empty() {
            blockers.push(prompt_missing_blocker());
        }

        let files = self
            .database
            .repository()
            .list_file_records(project_id)
            .await?;
        let tasks = files
            .into_iter()
            .filter(|file| file.included)
            .map(|file| {
                if file.content_hash.is_none() {
                    blockers.push(RunBlockingReason {
                        code: "validation_invalid_value",
                        message: "目标文件缺少最新内容哈希",
                        file_id: Some(file.id.clone()),
                        relative_path: Some(file.relative_path.clone()),
                    });
                }
                RunPreviewTask {
                    file_id: file.id,
                    relative_path: file.relative_path,
                    size_bytes: file.size_bytes,
                    content_hash: file.content_hash,
                }
            })
            .collect::<Vec<_>>();
        if tasks.is_empty() {
            blockers.push(no_target_files_blocker());
        }

        Ok(RunPreview {
            project_id: project_id.clone(),
            tasks,
            blockers,
            model,
            prompt_source: if input.prompt.is_some() {
                TaskValueSource::Override
            } else {
                TaskValueSource::Project
            },
            model_source: if input.model.is_some() {
                TaskValueSource::Override
            } else {
                TaskValueSource::Project
            },
            output_directory: project.output_root.unwrap_or_else(|| {
                Path::new(&project.source_directory)
                    .join(".batch-analysis")
                    .join("results")
                    .to_string_lossy()
                    .into_owned()
            }),
        })
    }
}

impl<'database> RunExecutionService<'database> {
    #[must_use]
    pub fn new(database: &'database Database, provider: OpenAiResponsesProvider) -> Self {
        Self { database, provider }
    }

    /// Executes queued Tasks sequentially and finalizes the Run.
    ///
    /// # Errors
    ///
    /// Returns a stable Run or persistence error. Provider and local input
    /// failures are persisted on their Attempt and do not abort the batch.
    #[allow(clippy::too_many_lines)]
    pub async fn execute(&self, run_id: &RunId) -> Result<Run, RunExecutionError> {
        let run = self
            .database
            .repository()
            .get_run(run_id)
            .await?
            .ok_or(RunExecutionError::NotFound)?;
        if run.status != RunStatus::Running {
            return Err(RunExecutionError::NotRunning);
        }
        let project = self
            .database
            .repository()
            .get_project(&run.project_id)
            .await?
            .ok_or(RunExecutionError::NotFound)?;
        let root = SafeRoot::new(&project.source_directory)
            .map_err(|_| RunExecutionError::PathUnavailable)?;

        // Validate the immutable Run snapshot and resolve the provider before
        // claiming a Task. A malformed or deleted profile must not leave a
        // claimed Task stuck in `running`.
        let profile_id = run
            .snapshot
            .api_routing
            .primary_profile_id
            .clone()
            .ok_or(RunExecutionError::NotRunning)?;
        let profile = self
            .database
            .repository()
            .get_api_profile(&profile_id)
            .await?
            .ok_or(RunExecutionError::NotFound)?;
        let secret_ref = profile
            .secret_ref
            .as_deref()
            .map(SecretRef::new)
            .ok_or(RunExecutionError::NotRunning)?;
        let provider_profile = ProviderApiProfile::new(
            batch_code_analyzer_api_profiles::ApiProfileId::new(profile.id.to_string()),
            profile.name.clone(),
            profile.base_url.clone(),
            secret_ref,
        )
        .map_err(|_| RunExecutionError::NotRunning)?
        .resolve();

        while let Some(mut task) = self
            .database
            .repository()
            .claim_next_task(run_id, &timestamp_now())
            .await?
        {
            let created_at = timestamp_now();
            let attempt_id = new_attempt_id();
            let sequence = u32::try_from(
                self.database
                    .repository()
                    .list_attempts(&task.id)
                    .await?
                    .len()
                    .saturating_add(1),
            )
            .unwrap_or(u32::MAX);
            let mut attempt = Attempt {
                id: attempt_id,
                task_id: task.id.clone(),
                sequence,
                api_profile_id: profile.id.clone(),
                api_profile_name: profile.name.clone(),
                actual_model: task.model_snapshot.clone(),
                status: AttemptStatus::Created,
                created_at: created_at.clone(),
                started_at: None,
                dispatched_at: None,
                finished_at: None,
                duration_ms: None,
                http_status: None,
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                retry_reason: None,
                error: None,
            };
            self.database
                .repository()
                .append_attempt(&attempt, AttemptRowMetadata { response_id: None })
                .await?;
            task.latest_attempt_id = Some(attempt.id.clone());
            attempt.status = AttemptStatus::Dispatched;
            attempt.started_at = Some(created_at.clone());
            attempt.dispatched_at = Some(created_at);
            self.database
                .repository()
                .finalize_task_attempt(&attempt, AttemptRowMetadata { response_id: None }, &task)
                .await?;
            let started = Instant::now();
            let source_path = root
                .relative_path(&task.relative_path)
                .map_err(|_| RunExecutionError::NotRunning)
                .map(|relative| root.path().join(relative));
            let source = match source_path {
                Ok(path) => fs::read_to_string(path).map_err(|_| "scan_file_unreadable"),
                Err(error) => Err(match error {
                    RunExecutionError::NotRunning => "project_path_unavailable",
                    _ => "scan_file_unreadable",
                }),
            };
            let result = match source {
                Ok(source) => {
                    let request = ProviderRequest::new(
                        provider_profile.clone(),
                        task.model_snapshot.clone(),
                        source,
                    )
                    .with_instructions(task.prompt_snapshot.clone())
                    .with_max_output_tokens(run.snapshot.max_output_tokens)
                    .with_timeout(Duration::from_secs(u64::from(run.snapshot.timeout_seconds)));
                    self.provider
                        .execute(request, CancellationToken::new())
                        .await
                        .map(|response| (response, started.elapsed()))
                        .map_err(|error| (error.code().to_owned(), error.retryable()))
                }
                Err(code) => Err((code.to_owned(), false)),
            };
            match result {
                Ok((response, elapsed)) => {
                    let Ok(output_path) = write_result(
                        &run.output_directory,
                        &task.relative_path,
                        &response.output_text,
                    ) else {
                        self.persist_task_failure(
                            &mut attempt,
                            &mut task,
                            "output_write_failed".into(),
                            true,
                            started,
                        )
                        .await?;
                        continue;
                    };
                    attempt.status = AttemptStatus::Succeeded;
                    attempt.finished_at = Some(timestamp_now());
                    attempt.duration_ms = Some(elapsed.as_millis().try_into().unwrap_or(u64::MAX));
                    attempt.input_tokens = response.usage.input_tokens.map(saturating_u32);
                    attempt.output_tokens = response.usage.output_tokens.map(saturating_u32);
                    attempt.total_tokens = response.usage.total_tokens.map(saturating_u32);
                    task.status =
                        TaskStateMachine::transition(task.status, TaskTransition::Succeed)
                            .map_err(|error| {
                                RunExecutionError::Persistence(PersistenceError::StateTransition {
                                    code: error.code(),
                                })
                            })?;
                    task.current_result_path = Some(output_path);
                    task.completed_at = attempt.finished_at.clone();
                    self.database
                        .repository()
                        .finalize_task_attempt(
                            &attempt,
                            AttemptRowMetadata {
                                response_id: response.response_id,
                            },
                            &task,
                        )
                        .await?;
                }
                Err((code, retryable)) => {
                    self.persist_task_failure(&mut attempt, &mut task, code, retryable, started)
                        .await?;
                }
            }
        }

        let stats = self
            .database
            .repository()
            .recompute_run_stats(run_id)
            .await?;
        let transition = if stats.failed == 0
            && stats.cancelled == 0
            && stats.interrupted == 0
            && stats.source_changed == 0
        {
            RunTransition::AllTasksSucceeded
        } else {
            RunTransition::AllTasksTerminalWithErrors
        };
        self.database
            .repository()
            .complete_run(run_id, transition, &timestamp_now())
            .await
            .map_err(Into::into)
    }

    async fn persist_task_failure(
        &self,
        attempt: &mut Attempt,
        task: &mut Task,
        code: String,
        retryable: bool,
        started: Instant,
    ) -> Result<(), RunExecutionError> {
        attempt.status = if retryable {
            AttemptStatus::FailedRetryable
        } else {
            AttemptStatus::FailedTerminal
        };
        attempt.finished_at = Some(timestamp_now());
        attempt.duration_ms = Some(started.elapsed().as_millis().try_into().unwrap_or(u64::MAX));
        attempt.error = Some(AttemptError {
            code,
            message: "模型请求未完成".into(),
            retryable,
            sanitized: true,
        });
        task.status =
            TaskStateMachine::transition(task.status, TaskTransition::Fail).map_err(|error| {
                RunExecutionError::Persistence(PersistenceError::StateTransition {
                    code: error.code(),
                })
            })?;
        task.completed_at = attempt.finished_at.clone();
        self.database
            .repository()
            .finalize_task_attempt(attempt, AttemptRowMetadata { response_id: None }, task)
            .await?;
        Ok(())
    }
}

fn resolve_prompt(input: &RunPreparationInput, project: &Project) -> String {
    input
        .prompt
        .clone()
        .unwrap_or_else(|| project.default_prompt.clone())
}

fn hash_prompt(prompt: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    prompt.hash(&mut hasher);
    format!("stable:{:016x}", hasher.finish())
}

fn new_run_id() -> RunId {
    let sequence = NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed);
    RunId::new(format!("run-{sequence}"))
}

fn new_task_id() -> TaskId {
    let sequence = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
    TaskId::new(format!("task-{sequence}"))
}

fn new_attempt_id() -> AttemptId {
    let sequence = NEXT_ATTEMPT_ID.fetch_add(1, Ordering::Relaxed);
    AttemptId::new(format!("attempt-{sequence}"))
}

fn new_context_version_id() -> ContextVersionId {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = NEXT_CONTEXT_VERSION_ID.fetch_add(1, Ordering::Relaxed);
    ContextVersionId::new(format!("context-{timestamp}-{sequence}"))
}

fn saturating_u32(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn write_result(
    output_directory: &str,
    relative_path: &str,
    content: &str,
) -> std::io::Result<String> {
    let directory = Path::new(output_directory);
    fs::create_dir_all(directory)?;
    let file_name = relative_path.replace(['/', '\\'], "__") + ".md";
    let target = directory.join(file_name);
    let temporary = target.with_extension(format!("md.tmp-{}", std::process::id()));
    fs::write(&temporary, content.as_bytes())?;
    fs::rename(&temporary, &target)?;
    Ok(target.to_string_lossy().into_owned())
}

const fn blocker(code: &'static str, message: &'static str) -> RunBlockingReason {
    RunBlockingReason {
        code,
        message,
        file_id: None,
        relative_path: None,
    }
}

const fn active_run_blocker() -> RunBlockingReason {
    blocker("run_active_exists", "当前已有活动 Run")
}

const fn api_profile_missing_blocker() -> RunBlockingReason {
    blocker("validation_required_field", "尚未配置主 API Profile")
}

const fn secret_missing_blocker() -> RunBlockingReason {
    blocker("security_secret_not_found", "主 API Profile 尚未配置密钥")
}

const fn model_missing_blocker() -> RunBlockingReason {
    blocker("validation_model_missing", "无法解析任务实际模型")
}

const fn prompt_missing_blocker() -> RunBlockingReason {
    blocker("validation_required_field", "提示词不能为空")
}

const fn no_target_files_blocker() -> RunBlockingReason {
    blocker("validation_required_field", "没有纳入本次 Run 的文件")
}

const fn path_unavailable_blocker() -> RunBlockingReason {
    blocker("project_path_unavailable", "项目路径不可用")
}

const fn invalid_run_blocker() -> RunBlockingReason {
    blocker("run_invalid_transition", "Run 状态转换无效")
}

const fn invalid_task_blocker() -> RunBlockingReason {
    blocker("task_invalid_transition", "Task 状态转换无效")
}

impl<'database> ContextService<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }

    /// Discovers root-level README/AGENTS files and stores a new immutable
    /// local `ContextVersion` without sending source content to a provider.
    ///
    /// # Errors
    ///
    /// Returns a stable context error without exposing absolute paths or file
    /// contents.
    pub async fn generate(
        &self,
        project_id: &ProjectId,
    ) -> Result<ContextVersion, ContextServiceError> {
        let project = self
            .database
            .repository()
            .get_project(project_id)
            .await?
            .ok_or(ContextServiceError::NotFound)?;
        if project.path_status != ProjectPathStatus::Available {
            return Err(ContextServiceError::PathUnavailable);
        }
        let root = SafeRoot::new(&project.source_directory)
            .map_err(|_| ContextServiceError::PathUnavailable)?;
        let mut discovered = Vec::new();
        let entries =
            fs::read_dir(root.path()).map_err(|_| ContextServiceError::DiscoveryFailed)?;
        for entry in entries {
            let entry = entry.map_err(|_| ContextServiceError::DiscoveryFailed)?;
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|_| ContextServiceError::DiscoveryFailed)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let lower = name.to_ascii_lowercase();
            if !(lower == "agents.md" || lower.starts_with("readme")) {
                continue;
            }
            if metadata.len() > DEFAULT_MAX_FILE_SIZE {
                continue;
            }
            let bytes = fs::read(entry.path()).map_err(|_| ContextServiceError::DiscoveryFailed)?;
            if bytes.contains(&0) {
                continue;
            }
            if std::str::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes))
                .is_err()
            {
                continue;
            }
            discovered.push((name, metadata.len(), content_hash(&bytes)));
        }
        discovered.sort_by(|left, right| left.0.cmp(&right.0));
        let source_files = discovered
            .iter()
            .map(
                |(relative_path, _, content_hash)| ContextVersionSourceFile {
                    relative_path: relative_path.clone(),
                    content_hash: content_hash.clone(),
                    included: true,
                    truncated: false,
                },
            )
            .collect::<Vec<_>>();
        let mut summary = format!("本地发现 {} 个项目上下文文件。\n\n", source_files.len());
        for (relative_path, size_bytes, _) in &discovered {
            let _ = writeln!(summary, "- {relative_path} ({size_bytes} bytes)");
        }
        summary.push_str("\n当前摘要由本地发现生成，尚未调用模型。\n");
        let context_version = ContextVersion {
            id: new_context_version_id(),
            project_id: project_id.clone(),
            status: ContextStatus::Ready,
            source_files,
            model: project
                .context_model
                .clone()
                .or(project.default_model.clone()),
            summary_hash: content_hash(summary.as_bytes()),
            summary,
            manually_edited: false,
            created_at: timestamp_now(),
        };
        self.database
            .repository()
            .create_context_version_and_update_project(
                &context_version,
                true,
                ContextStatus::Ready,
                &timestamp_now(),
            )
            .await?;
        Ok(context_version)
    }

    /// Gets the current `ContextVersion` referenced by a project.
    ///
    /// # Errors
    ///
    /// Returns a stable persistence error when the project or version cannot
    /// be read.
    pub async fn get(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<ContextVersion>, ContextServiceError> {
        let project = self
            .database
            .repository()
            .get_project(project_id)
            .await?
            .ok_or(ContextServiceError::NotFound)?;
        let Some(version_id) = project.project_context.current_version_id else {
            return Ok(None);
        };
        self.database
            .repository()
            .get_context_version(&version_id)
            .await
            .map_err(Into::into)
    }
}

impl<'database> ApiProfileService<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }

    /// Lists persisted API profile metadata.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the database cannot be read.
    pub async fn list(&self) -> Result<Vec<ApiProfile>, PersistenceError> {
        self.database.repository().list_api_profiles().await
    }

    /// Loads one API profile.
    ///
    /// # Errors
    ///
    /// Returns a stable service error when the profile is missing or storage
    /// cannot be read.
    pub async fn get(
        &self,
        profile_id: &ApiProfileId,
    ) -> Result<ApiProfile, ApiProfileServiceError> {
        self.database
            .repository()
            .get_api_profile(profile_id)
            .await?
            .ok_or(ApiProfileServiceError::NotFound)
    }

    /// Creates or updates validated non-sensitive API profile metadata.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the profile is invalid
    /// or cannot be stored.
    pub async fn save(
        &self,
        profile_id: Option<ApiProfileId>,
        name: String,
        base_url: String,
        default_model: Option<String>,
    ) -> Result<ApiProfile, ApiProfileServiceError> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err(ApiProfileServiceError::InvalidName);
        }
        let base_url = normalize_profile_base_url(&base_url)?;
        let now = timestamp_now();
        if let Some(profile_id) = profile_id {
            let mut profile = self.get(&profile_id).await?;
            profile.name = name;
            profile.base_url = base_url;
            profile.default_model = default_model;
            profile.updated_at = now;
            self.database
                .repository()
                .update_api_profile(&profile)
                .await?;
            return Ok(profile);
        }

        let profile = ApiProfile {
            id: new_api_profile_id(),
            name,
            protocol: ApiProtocol::OpenAiResponses,
            base_url,
            secret_ref: None,
            default_model,
            model_cache: Vec::new(),
            model_cache_updated_at: None,
            last_connection_status: ApiProfileConnectionStatus::Unknown,
            last_error_code: None,
            last_tested_at: None,
            created_at: now.clone(),
            updated_at: now,
        };
        self.database
            .repository()
            .create_api_profile(&profile)
            .await?;
        Ok(profile)
    }

    /// Associates an opaque `SecretStore` reference with a profile.
    ///
    /// # Errors
    ///
    /// Returns a stable service error when the profile is missing or storage
    /// cannot be updated.
    pub async fn set_secret_ref(
        &self,
        profile_id: &ApiProfileId,
        secret_ref: Option<String>,
    ) -> Result<ApiProfile, ApiProfileServiceError> {
        let mut profile = self.get(profile_id).await?;
        profile.secret_ref = secret_ref;
        profile.updated_at = timestamp_now();
        self.database
            .repository()
            .update_api_profile(&profile)
            .await?;
        Ok(profile)
    }

    /// Persists a sanitized connection result and optional model cache.
    ///
    /// # Errors
    ///
    /// Returns a stable service error when the profile is missing or storage
    /// cannot be updated.
    pub async fn set_test_result(
        &self,
        profile_id: &ApiProfileId,
        status: ApiProfileConnectionStatus,
        error_code: Option<String>,
        models: Vec<ApiModelInfo>,
    ) -> Result<ApiProfile, ApiProfileServiceError> {
        let mut profile = self.get(profile_id).await?;
        let now = timestamp_now();
        profile.last_connection_status = status;
        profile.last_error_code = error_code;
        profile.last_tested_at = Some(now.clone());
        profile.updated_at = now;
        if !models.is_empty() {
            profile.model_cache = models;
            profile.model_cache_updated_at = profile.last_tested_at.clone();
        }
        self.database
            .repository()
            .update_api_profile(&profile)
            .await?;
        Ok(profile)
    }

    /// Deletes a profile that is not referenced by a project.
    ///
    /// # Errors
    ///
    /// Returns `api_profile_in_use`, a not-found error, or a persistence error.
    pub async fn delete(&self, profile_id: &ApiProfileId) -> Result<(), ApiProfileServiceError> {
        self.database
            .repository()
            .delete_api_profile(profile_id)
            .await
            .map_err(Into::into)
    }
}

fn normalize_profile_base_url(value: &str) -> Result<String, ApiProfileServiceError> {
    let trimmed = value.trim().trim_end_matches('/');
    let parsed = url::Url::parse(trimmed).map_err(|_| ApiProfileServiceError::InvalidBaseUrl)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(ApiProfileServiceError::InvalidBaseUrl);
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(ApiProfileServiceError::UrlContainsCredentials);
    }
    Ok(trimmed.to_owned())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectConfigMirror<'project> {
    schema_version: u32,
    id: &'project ProjectId,
    name: &'project str,
    source_directory: &'project str,
    default_prompt: &'project str,
    default_model: &'project Option<String>,
    context_model: &'project Option<String>,
    api_routing: &'project ApiRouting,
    output_root: &'project Option<String>,
}

fn write_project_mirror(project: &Project) -> std::io::Result<()> {
    let directory = Path::new(&project.source_directory).join(".batch-analysis");
    fs::create_dir_all(&directory)?;
    let temporary = directory.join(format!("project.json.tmp-{}", std::process::id()));
    let target = directory.join("project.json");
    let mirror = ProjectConfigMirror {
        schema_version: project.schema_version,
        id: &project.id,
        name: &project.name,
        source_directory: &project.source_directory,
        default_prompt: &project.default_prompt,
        default_model: &project.default_model,
        context_model: &project.context_model,
        api_routing: &project.api_routing,
        output_root: &project.output_root,
    };
    let bytes = serde_json::to_vec_pretty(&mirror).map_err(std::io::Error::other)?;
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, target)
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("未命名项目")
        .to_owned()
}

fn new_project_id() -> ProjectId {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = NEXT_PROJECT_ID.fetch_add(1, Ordering::Relaxed);
    ProjectId::new(format!("project-{timestamp}-{sequence}"))
}

fn new_file_id() -> FileRecordId {
    let sequence = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
    FileRecordId::new(format!("file-{sequence}"))
}

fn new_api_profile_id() -> ApiProfileId {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = NEXT_API_PROFILE_ID.fetch_add(1, Ordering::Relaxed);
    ApiProfileId::new(format!("profile-{timestamp}-{sequence}"))
}

#[must_use]
pub fn timestamp_now() -> Rfc3339Timestamp {
    let value = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into());
    Rfc3339Timestamp::new(value)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, sync::Arc, time::Duration};

    use batch_code_analyzer_domain::ApiProfileId;
    use batch_code_analyzer_model_providers::OpenAiResponsesProvider;
    use batch_code_analyzer_persistence::Database;
    use batch_code_analyzer_repository_scanner::ScanCancellation;
    use batch_code_analyzer_secret_store::{MemorySecretStore, SecretStore, SecretValue};
    use tokio::{io::AsyncWriteExt, net::TcpListener};

    use super::{
        timestamp_now, ApiProfileService, ApiProfileServiceError, ContextService, FileServiceError,
        ProjectService, ProjectServiceError, RunExecutionService, RunPreparationInput, RunService,
        ScanService,
    };

    fn temporary_directory(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "batch-code-analyzer-project-service-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temporary directory should be created");
        path
    }

    async fn provider_server(status: &str, body: &str) -> String {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("provider server should bind");
        let address = listener.local_addr().expect("provider address");
        let status = status.to_owned();
        let body = body.to_owned();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("provider request");
            let mut request = [0_u8; 4096];
            let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut request).await;
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("provider response should be written");
        });
        format!("http://{address}/v1")
    }

    async fn execution_fixture(
        status: &str,
        body: &str,
        output_blocked: bool,
    ) -> (
        Database,
        batch_code_analyzer_domain::Run,
        OpenAiResponsesProvider,
        Arc<MemorySecretStore>,
        PathBuf,
    ) {
        let database = Database::open_in_memory()
            .await
            .expect("database should open");
        let path = temporary_directory(if status.starts_with("200") {
            if output_blocked {
                "run-execution-output-failure"
            } else {
                "run-execution-success"
            }
        } else {
            "run-execution-failure"
        });
        fs::write(path.join("main.rs"), "fn main() {}\n").expect("source file should be created");
        let mut project = ProjectService::new(&database)
            .add_project(&path)
            .await
            .expect("project should add")
            .project;
        let output_path_file = path.join("output-blocker");
        if output_blocked {
            fs::write(
                &output_path_file,
                b"output path is intentionally unavailable",
            )
            .expect("output blocker should be created");
            project.output_root = Some(output_path_file.to_string_lossy().into_owned());
        }
        let base_url = provider_server(status, body).await;
        let profile = ApiProfileService::new(&database)
            .save(None, "Test Profile".into(), base_url, Some("gpt-5".into()))
            .await
            .expect("profile should save");
        let secrets = Arc::new(MemorySecretStore::new());
        let secret_ref = secrets
            .put(SecretValue::new("sk-test-key"))
            .await
            .expect("secret should be stored");
        ApiProfileService::new(&database)
            .set_secret_ref(&profile.id, Some(secret_ref.to_string()))
            .await
            .expect("profile secret should be linked");
        project.api_routing.primary_profile_id = Some(profile.id);
        database
            .repository()
            .update_project(
                &project,
                batch_code_analyzer_persistence::ProjectRowMetadata {
                    canonical_source_directory: path.to_string_lossy().into_owned(),
                    created_at: timestamp_now(),
                    updated_at: timestamp_now(),
                },
            )
            .await
            .expect("project routing should save");
        let scan = ScanService::scan_project(&project, ScanCancellation::new())
            .expect("scan should complete");
        ScanService::new(&database)
            .persist_scan(&project.id, scan)
            .await
            .expect("scan should persist");
        let run = RunService::new(&database)
            .create(&project.id, &RunPreparationInput::default())
            .await
            .expect("run should create");
        let provider = OpenAiResponsesProvider::with_client(
            reqwest::Client::new(),
            secrets.clone(),
            Duration::from_secs(2),
        );
        (database, run, provider, secrets, path)
    }

    #[tokio::test]
    async fn run_execution_persists_successful_attempt_and_result() {
        let (database, run, provider, _secrets, path) = execution_fixture(
            "200 OK",
            r##"{"id":"resp-1","output_text":"# Result"}"##,
            false,
        )
        .await;
        let completed = RunExecutionService::new(&database, provider)
            .execute(&run.id)
            .await
            .expect("run should execute");
        assert_eq!(
            completed.status,
            batch_code_analyzer_domain::RunStatus::Completed
        );
        assert_eq!(completed.stats.succeeded, 1);
        assert_eq!(completed.stats.failed, 0);
        let tasks = database
            .repository()
            .unfinished_tasks(&run.id)
            .await
            .unwrap();
        assert!(tasks.is_empty(), "successful task must not remain running");
        let task_id = database.repository().list_tasks(&run.id).await.unwrap()[0]
            .id
            .clone();
        let attempts = database.repository().list_attempts(&task_id).await.unwrap();
        assert_eq!(attempts.len(), 1);
        assert_eq!(
            attempts[0].status,
            batch_code_analyzer_domain::AttemptStatus::Succeeded
        );
        assert!(PathBuf::from(completed.output_directory)
            .join("main.rs.md")
            .is_file());
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn run_execution_persists_retryable_provider_failure() {
        let (database, run, provider, _secrets, path) = execution_fixture(
            "500 Internal Server Error",
            r#"{"error":{"message":"temporary"}}"#,
            false,
        )
        .await;
        let completed = RunExecutionService::new(&database, provider)
            .execute(&run.id)
            .await
            .expect("provider failure should be persisted");
        assert_eq!(
            completed.status,
            batch_code_analyzer_domain::RunStatus::CompletedWithErrors
        );
        assert_eq!(completed.stats.failed, 1);
        assert!(database
            .repository()
            .unfinished_tasks(&run.id)
            .await
            .unwrap()
            .is_empty());
        let task_id = database.repository().list_tasks(&run.id).await.unwrap()[0]
            .id
            .clone();
        let attempts = database.repository().list_attempts(&task_id).await.unwrap();
        assert_eq!(attempts.len(), 1);
        assert_eq!(
            attempts[0].status,
            batch_code_analyzer_domain::AttemptStatus::FailedRetryable
        );
        assert_eq!(
            attempts[0].error.as_ref().map(|error| error.code.as_str()),
            Some("provider_server_error")
        );
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn run_execution_persists_output_write_failure_without_success_state() {
        let (database, run, provider, _secrets, path) = execution_fixture(
            "200 OK",
            r##"{"id":"resp-2","output_text":"# Result"}"##,
            true,
        )
        .await;
        let completed = RunExecutionService::new(&database, provider)
            .execute(&run.id)
            .await
            .expect("output failure should be persisted");
        assert_eq!(
            completed.status,
            batch_code_analyzer_domain::RunStatus::CompletedWithErrors
        );
        let task = database.repository().list_tasks(&run.id).await.unwrap()[0].clone();
        assert_eq!(task.status, batch_code_analyzer_domain::TaskStatus::Failed);
        assert_eq!(task.current_result_path, None);
        let attempts = database.repository().list_attempts(&task.id).await.unwrap();
        assert_eq!(attempts.len(), 1);
        assert_eq!(
            attempts[0].status,
            batch_code_analyzer_domain::AttemptStatus::FailedRetryable
        );
        assert_eq!(
            attempts[0].error.as_ref().map(|error| error.code.as_str()),
            Some("output_write_failed")
        );
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn adds_project_writes_mirror_and_returns_existing_project_for_duplicate_path() {
        let database = Database::open_in_memory()
            .await
            .expect("database should open");
        let path = temporary_directory("duplicate");
        let service = ProjectService::new(&database);

        let first = service
            .add_project(&path)
            .await
            .expect("project should add");
        assert!(first.created);
        assert!(!first.config_mirror_warning);
        assert!(first.project.name.ends_with("-duplicate"));
        assert!(path.join(".batch-analysis/project.json").is_file());
        assert!(first.project.api_routing.primary_profile_id.is_none());

        let second = service
            .add_project(&path)
            .await
            .expect("duplicate should resolve");
        assert!(!second.created);
        assert_eq!(second.project.id, first.project.id);

        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn rejects_missing_paths_without_exposing_filesystem_details() {
        let database = Database::open_in_memory()
            .await
            .expect("database should open");
        let service = ProjectService::new(&database);
        let error = service
            .add_project("/path/that/does/not/exist")
            .await
            .expect_err("missing path should fail");
        assert_eq!(error, ProjectServiceError::PathUnavailable);
        assert_eq!(error.to_string(), "project_path_unavailable");
    }

    #[tokio::test]
    async fn mirror_failure_does_not_rollback_the_project_registration() {
        let database = Database::open_in_memory()
            .await
            .expect("database should open");
        let path = temporary_directory("readonly-mirror");
        fs::write(path.join(".batch-analysis"), b"not a directory")
            .expect("mirror blocker should be created");
        let service = ProjectService::new(&database);

        let result = service
            .add_project(&path)
            .await
            .expect("SQLite registration should succeed");

        assert!(result.created);
        assert!(result.config_mirror_warning);
        assert_eq!(service.list_projects().await.unwrap(), vec![result.project]);

        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn updates_project_run_settings_and_rejects_unknown_profiles() {
        let database = Database::open_in_memory()
            .await
            .expect("database should open");
        let path = temporary_directory("run-settings");
        let project = ProjectService::new(&database)
            .add_project(&path)
            .await
            .expect("project should add")
            .project;
        let profile = ApiProfileService::new(&database)
            .save(
                None,
                "Local API".into(),
                "https://example.test/v1".into(),
                Some("gpt-5".into()),
            )
            .await
            .expect("profile should save");
        let service = ProjectService::new(&database);
        let updated = service
            .update_run_settings(
                &project.id,
                Some(profile.id.clone()),
                Some("gpt-5-mini".into()),
            )
            .await
            .expect("project settings should update");
        assert_eq!(
            updated.project.api_routing.primary_profile_id,
            Some(profile.id.clone())
        );
        assert_eq!(updated.project.default_model.as_deref(), Some("gpt-5-mini"));
        let persisted = database
            .repository()
            .get_project(&project.id)
            .await
            .unwrap()
            .expect("project should remain persisted");
        assert_eq!(persisted, updated.project);
        let mirror = fs::read_to_string(path.join(".batch-analysis/project.json"))
            .expect("project mirror should exist");
        assert!(mirror.contains("apiRouting"));
        assert!(!mirror.contains("sk-test"));
        assert_eq!(
            service
                .update_run_settings(
                    &project.id,
                    Some(ApiProfileId::new("missing-profile")),
                    Some("gpt-5".into()),
                )
                .await
                .expect_err("unknown profile should be rejected")
                .code(),
            "validation_invalid_value"
        );
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn concurrent_registration_returns_the_same_project_for_a_duplicate_path() {
        let database = Database::open_in_memory()
            .await
            .expect("database should open");
        let path = temporary_directory("concurrent");
        let service = ProjectService::new(&database);

        let (first, second) = tokio::join!(service.add_project(&path), service.add_project(&path));
        let first = first.expect("first registration should complete");
        let second = second.expect("duplicate registration should complete");

        assert_ne!(first.created, second.created);
        assert_eq!(first.project.id, second.project.id);
        assert_eq!(service.list_projects().await.unwrap().len(), 1);

        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn run_preview_and_create_freeze_scanned_files_and_reject_an_active_run() {
        let database = Database::open_in_memory()
            .await
            .expect("database should open");
        let path = temporary_directory("run-preview");
        fs::write(path.join("main.rs"), "fn main() {}\n").expect("source file should be created");
        let mut project = ProjectService::new(&database)
            .add_project(&path)
            .await
            .expect("project should add")
            .project;
        let profile = ApiProfileService::new(&database)
            .save(
                None,
                "Test Profile".into(),
                "https://example.test/v1".into(),
                Some("gpt-5".into()),
            )
            .await
            .expect("profile should save");
        ApiProfileService::new(&database)
            .set_secret_ref(&profile.id, Some("session-secret-1".into()))
            .await
            .expect("secret reference should save");
        project.api_routing.primary_profile_id = Some(profile.id);
        database
            .repository()
            .update_project(
                &project,
                batch_code_analyzer_persistence::ProjectRowMetadata {
                    canonical_source_directory: path.to_string_lossy().into_owned(),
                    created_at: timestamp_now(),
                    updated_at: timestamp_now(),
                },
            )
            .await
            .expect("project routing should save");
        let scan = ScanService::scan_project(&project, ScanCancellation::new())
            .expect("scan should complete");
        ScanService::new(&database)
            .persist_scan(&project.id, scan)
            .await
            .expect("scan should persist");

        let service = RunService::new(&database);
        let preview = service
            .preview(&project.id, &RunPreparationInput::default())
            .await
            .expect("preview should build");
        assert_eq!(preview.tasks.len(), 1);
        assert!(preview.blockers.is_empty());
        assert_eq!(preview.model.as_deref(), Some("gpt-5"));

        let run = service
            .create(&project.id, &RunPreparationInput::default())
            .await
            .expect("run should create");
        assert_eq!(run.status, batch_code_analyzer_domain::RunStatus::Running);
        assert_eq!(run.stats.queued, 1);
        assert_eq!(
            service
                .create(&project.id, &RunPreparationInput::default())
                .await
                .expect_err("second active run must be rejected")
                .code(),
            "run_active_exists"
        );

        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn completed_scans_persist_files_and_mark_missing_files_deleted() {
        let database = Database::open_in_memory()
            .await
            .expect("database should open");
        let path = temporary_directory("scan");
        fs::write(path.join("main.rs"), "fn main() {}\n").expect("source file should be created");
        let project = ProjectService::new(&database)
            .add_project(&path)
            .await
            .expect("project should add")
            .project;
        let service = ScanService::new(&database);

        let first_result = ScanService::scan_project(&project, ScanCancellation::new())
            .expect("scan should complete");
        let first_summary = service
            .persist_scan(&project.id, first_result)
            .await
            .expect("scan generation should commit");
        assert_eq!(first_summary.generation, 1);
        assert!(first_summary.report.included_files >= 1);
        assert!(database
            .repository()
            .list_file_records(&project.id)
            .await
            .unwrap()
            .iter()
            .any(|file| file.relative_path == "main.rs" && file.included));

        fs::remove_file(path.join("main.rs")).expect("source file should be removed");
        let second_result = ScanService::scan_project(&project, ScanCancellation::new())
            .expect("second scan should complete");
        let second_summary = service
            .persist_scan(&project.id, second_result)
            .await
            .expect("second scan generation should commit");
        assert_eq!(second_summary.generation, 2);
        assert_eq!(
            database
                .repository()
                .list_file_records(&project.id)
                .await
                .unwrap()
                .iter()
                .find(|file| file.relative_path == "main.rs")
                .unwrap()
                .source_status,
            batch_code_analyzer_domain::FileSourceStatus::Deleted
        );

        let cancellation = ScanCancellation::new();
        cancellation.cancel();
        let cancelled = ScanService::scan_project(&project, cancellation)
            .expect("cancelled scan should return diagnostics");
        assert!(!cancelled.completed);
        assert_eq!(
            service
                .persist_scan(&project.id, cancelled)
                .await
                .expect_err("cancelled scan must not be persisted")
                .to_string(),
            "scan_cancelled"
        );

        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn context_generation_discovers_root_documents_and_updates_current_version() {
        let database = Database::open_in_memory()
            .await
            .expect("database should open");
        let path = temporary_directory("context");
        fs::write(path.join("README.md"), "# InkOS\n\nProject overview\n")
            .expect("README should be created");
        fs::write(path.join("AGENTS.md"), "# Rules\nKeep source local.\n")
            .expect("AGENTS should be created");
        fs::write(path.join("README.tmp"), [0, 1, 2]).expect("binary fixture should be created");
        let project = ProjectService::new(&database)
            .add_project(&path)
            .await
            .expect("project should add")
            .project;

        let service = ContextService::new(&database);
        let context = service
            .generate(&project.id)
            .await
            .expect("context should generate");
        assert_eq!(context.source_files.len(), 2);
        assert_eq!(context.source_files[0].relative_path, "AGENTS.md");
        assert_eq!(context.source_files[1].relative_path, "README.md");
        assert!(!context.summary.contains("Project overview"));

        let current = service
            .get(&project.id)
            .await
            .expect("context should load")
            .expect("current context should exist");
        assert_eq!(current.id, context.id);
        let updated_project = database
            .repository()
            .get_project(&project.id)
            .await
            .unwrap()
            .expect("project should load");
        assert_eq!(
            updated_project.project_context.current_version_id.as_ref(),
            Some(&context.id)
        );

        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn file_inclusion_override_survives_rescan_and_blocks_sensitive_files() {
        let database = Database::open_in_memory()
            .await
            .expect("database should open");
        let path = temporary_directory("file-inclusion");
        fs::write(path.join("main.rs"), "fn main() {}\n").expect("source file should be created");
        let project = ProjectService::new(&database)
            .add_project(&path)
            .await
            .expect("project should add")
            .project;
        let service = ScanService::new(&database);
        let first = ScanService::scan_project(&project, ScanCancellation::new())
            .expect("scan should complete");
        service
            .persist_scan(&project.id, first)
            .await
            .expect("scan should persist");
        let main = database
            .repository()
            .list_file_records(&project.id)
            .await
            .unwrap()
            .into_iter()
            .find(|file| file.relative_path == "main.rs")
            .expect("main file should exist");

        let excluded = ProjectService::new(&database)
            .set_file_included(&project.id, &main.id, false)
            .await
            .expect("file should be excluded");
        assert!(!excluded.included);
        assert_eq!(excluded.exclusion_reason.as_deref(), Some("user_excluded"));

        let second = ScanService::scan_project(&project, ScanCancellation::new())
            .expect("second scan should complete");
        service
            .persist_scan(&project.id, second)
            .await
            .expect("second scan should persist");
        let persisted = database
            .repository()
            .get_file_record(&main.id)
            .await
            .unwrap()
            .expect("excluded file should remain");
        assert!(!persisted.included);
        assert_eq!(persisted.exclusion_reason.as_deref(), Some("user_excluded"));

        let restored = ProjectService::new(&database)
            .set_file_included(&project.id, &main.id, true)
            .await
            .expect("normal file should be restored");
        assert!(restored.included);

        fs::write(path.join(".env"), "API_KEY=not-a-real-secret-value\n")
            .expect("sensitive fixture should be created");
        let third = ScanService::scan_project(&project, ScanCancellation::new())
            .expect("third scan should complete");
        service
            .persist_scan(&project.id, third)
            .await
            .expect("third scan should persist");
        let sensitive = database
            .repository()
            .list_file_records(&project.id)
            .await
            .unwrap()
            .into_iter()
            .find(|file| file.relative_path == ".env")
            .expect("sensitive file should be recorded");
        assert_eq!(
            ProjectService::new(&database)
                .set_file_included(&project.id, &sensitive.id, true)
                .await
                .expect_err("sensitive file should remain blocked"),
            FileServiceError::SensitiveBlocked
        );

        let authorized = ProjectService::new(&database)
            .authorize_sensitive_file(&project.id, &sensitive.id, true)
            .await
            .expect("explicit sensitive authorization should succeed");
        assert!(authorized.included);
        assert_eq!(
            authorized.source_status,
            batch_code_analyzer_domain::FileSourceStatus::Sensitive
        );
        assert_eq!(
            authorized.exclusion_reason.as_deref(),
            Some("user_authorized_sensitive")
        );
        assert!(authorized.content_hash.is_some());

        let revoked = ProjectService::new(&database)
            .set_file_included(&project.id, &sensitive.id, false)
            .await
            .expect("sensitive authorization should be revocable");
        assert!(!revoked.included);

        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn api_profile_service_validates_and_persists_metadata_without_secrets() {
        let database = Database::open_in_memory()
            .await
            .expect("database should open");
        let service = ApiProfileService::new(&database);
        assert_eq!(
            service
                .save(None, "Profile".into(), "ftp://example.test".into(), None,)
                .await
                .expect_err("unsupported protocol should fail"),
            ApiProfileServiceError::InvalidBaseUrl
        );
        let profile = service
            .save(
                None,
                "Profile".into(),
                "https://example.test/v1/".into(),
                Some("gpt-5".into()),
            )
            .await
            .expect("profile should save");
        assert_eq!(profile.base_url, "https://example.test/v1");
        assert_eq!(profile.secret_ref, None);
        let with_secret_ref = service
            .set_secret_ref(&profile.id, Some("session-secret-1".into()))
            .await
            .expect("secret reference should update");
        assert_eq!(
            with_secret_ref.secret_ref.as_deref(),
            Some("session-secret-1")
        );
    }
}
