//! Application-layer orchestration for Batch Code Analyzer.

#![forbid(unsafe_code)]

use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use batch_code_analyzer_domain::{
    ApiRouting, ContextStatus, ExecutionDefaults, FilterRules, Project, ProjectContext, ProjectId,
    ProjectPathStatus, Rfc3339Timestamp,
};
use batch_code_analyzer_persistence::{
    Database, PersistenceError, ProjectRowMetadata, LATEST_SCHEMA_VERSION,
};
use batch_code_analyzer_security_core::SafeRoot;
use serde::Serialize;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub use batch_code_analyzer_domain as domain;

const DEFAULT_PROMPT: &str = "请结合提供的项目上下文，用通俗但准确的语言解释当前代码文件。\n请说明：\n1. 该文件在项目中的核心职责；\n2. 关键输入、输出、状态或数据流；\n3. 它与哪些模块或功能协作，以及它为何存在；\n4. 修改或缺失它可能带来的影响。\n如无法从上下文或代码中确认，请明确说明不确定性，不要臆测。";
static NEXT_PROJECT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Eq, PartialEq)]
pub enum ProjectServiceError {
    PathUnavailable,
    Persistence(PersistenceError),
}

impl ProjectServiceError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
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

#[derive(Debug)]
pub struct ProjectAddResult {
    pub project: Project,
    pub created: bool,
    pub config_mirror_warning: bool,
}

pub struct ProjectService<'database> {
    database: &'database Database,
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

    /// Lists project summaries from the `SQLite` source of truth.
    ///
    /// # Errors
    ///
    /// Returns a stable persistence error when the list cannot be read.
    pub async fn list_projects(&self) -> Result<Vec<Project>, PersistenceError> {
        self.database.repository().list_projects().await
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

fn timestamp_now() -> Rfc3339Timestamp {
    let value = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into());
    Rfc3339Timestamp::new(value)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use batch_code_analyzer_persistence::Database;

    use super::{ProjectService, ProjectServiceError};

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
}
