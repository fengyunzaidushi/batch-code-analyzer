//! Transactional repositories for persisted domain entities.

use batch_code_analyzer_domain::{
    Attempt, AttemptId, ContextVersion, ContextVersionId, FileRecord, FileRecordId, Project,
    ProjectId, Run, RunId, RunStats, RunStatus, Task, TaskId, TaskStateMachine, TaskStatus,
    TaskTransition,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use sqlx::{sqlite::SqliteRow, Row};

use crate::{
    AttemptRow, AttemptRowMetadata, ContextVersionRow, Database, FileRecordRow,
    FileRecordRowMetadata, PersistenceError, ProjectRow, ProjectRowMetadata, RunRow,
    RunRowMetadata, TaskRow, WriteTransaction,
};

/// Transactional access to the persistence model.
///
/// Repositories deliberately return Domain entities. Callers that cross the
/// application boundary must convert those entities to IPC DTOs separately.
#[derive(Clone, Copy, Debug)]
pub struct Repository<'database> {
    database: &'database Database,
}

impl Database {
    /// Creates a repository facade backed by this database.
    #[must_use]
    pub const fn repository(&self) -> Repository<'_> {
        Repository { database: self }
    }

    /// Alias retained for callers that group multiple repositories together.
    #[must_use]
    pub const fn repositories(&self) -> Repository<'_> {
        self.repository()
    }
}

impl Repository<'_> {
    /// Inserts a Project and enforces the canonical path uniqueness constraint.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the entity cannot be encoded, the
    /// canonical path is already registered, or the transaction fails.
    pub async fn create_project(
        &self,
        project: &Project,
        metadata: ProjectRowMetadata,
    ) -> Result<(), PersistenceError> {
        let row = ProjectRow::from_domain(project, metadata)?;
        let mut transaction = self.database.begin_write().await?;
        let result = insert_project(&mut transaction, &row).await;
        finish_write(transaction, result).await
    }

    /// Loads a Project without exposing its `SQLite` row representation.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the transaction or stored row cannot
    /// be read or decoded.
    pub async fn get_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<Project>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = load_project(&mut transaction, project_id.as_str()).await;
        finish_read(transaction, result).await
    }

    /// Finds a Project by its canonical source directory.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the query or stored row cannot be
    /// read or decoded.
    pub async fn find_project_by_canonical_path(
        &self,
        canonical_path: &str,
    ) -> Result<Option<Project>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = sqlx::query(
            "SELECT id, schema_version, name, source_directory,
                canonical_source_directory, path_status, default_prompt,
                default_model, context_model, output_root, filter_rules_json,
                execution_defaults_json, api_routing_json,
                current_context_version_id, context_enabled, context_status,
                created_at, updated_at, last_opened_at
             FROM projects WHERE canonical_source_directory = ?",
        )
        .bind(canonical_path)
        .fetch_optional(transaction.connection())
        .await
        .map_err(|_| PersistenceError::TransactionFailed)?
        .map(|row| project_from_row(&row))
        .transpose();
        finish_read(transaction, result).await
    }

    /// Lists Projects in stable name/ID order.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the query or stored row decoding fails.
    pub async fn list_projects(&self) -> Result<Vec<Project>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = async {
            let rows = sqlx::query(
                "SELECT id, schema_version, name, source_directory,
                    canonical_source_directory, path_status, default_prompt,
                    default_model, context_model, output_root, filter_rules_json,
                    execution_defaults_json, api_routing_json,
                    current_context_version_id, context_enabled, context_status,
                    created_at, updated_at, last_opened_at
                 FROM projects ORDER BY name ASC, id ASC",
            )
            .fetch_all(transaction.connection())
            .await
            .map_err(|_| PersistenceError::TransactionFailed)?;

            rows.into_iter().map(|row| project_from_row(&row)).collect()
        }
        .await;
        finish_read(transaction, result).await
    }

    /// Updates mutable Project settings and its canonical path atomically.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the project is missing, its path is
    /// duplicated, or the transaction fails.
    pub async fn update_project(
        &self,
        project: &Project,
        metadata: ProjectRowMetadata,
    ) -> Result<(), PersistenceError> {
        let row = ProjectRow::from_domain(project, metadata)?;
        let mut transaction = self.database.begin_write().await?;
        let result = async {
            let affected = sqlx::query(
                "UPDATE projects SET schema_version = ?, name = ?,
                    source_directory = ?, canonical_source_directory = ?,
                    path_status = ?, default_prompt = ?, default_model = ?,
                    context_model = ?, output_root = ?, filter_rules_json = ?,
                    execution_defaults_json = ?, api_routing_json = ?,
                    current_context_version_id = ?, context_enabled = ?,
                    context_status = ?, updated_at = ?, last_opened_at = ?
                 WHERE id = ?",
            )
            .bind(row.schema_version)
            .bind(row.name)
            .bind(row.source_directory)
            .bind(row.canonical_source_directory)
            .bind(row.path_status)
            .bind(row.default_prompt)
            .bind(row.default_model)
            .bind(row.context_model)
            .bind(row.output_root)
            .bind(row.filter_rules_json)
            .bind(row.execution_defaults_json)
            .bind(row.api_routing_json)
            .bind(row.current_context_version_id)
            .bind(row.context_enabled)
            .bind(row.context_status)
            .bind(row.updated_at)
            .bind(row.last_opened_at)
            .bind(row.id)
            .execute(transaction.connection())
            .await
            .map_err(|error| map_project_write_error(&error))?
            .rows_affected();

            if affected == 0 {
                return Err(project_not_found(project.id.as_str()));
            }

            Ok(())
        }
        .await;
        finish_write(transaction, result).await
    }

    /// Inserts a scanner-produced `FileRecord`.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the row cannot be encoded, its project
    /// is missing, or the transaction fails.
    pub async fn create_file_record(
        &self,
        file_record: &FileRecord,
        metadata: FileRecordRowMetadata,
    ) -> Result<(), PersistenceError> {
        let row = FileRecordRow::from_domain(file_record, metadata)?;
        let mut transaction = self.database.begin_write().await?;
        let result = insert_file_record(&mut transaction, &row).await;
        finish_write(transaction, result).await
    }

    /// Loads one `FileRecord` by ID.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the query or stored row decoding fails.
    pub async fn get_file_record(
        &self,
        file_id: &FileRecordId,
    ) -> Result<Option<FileRecord>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = load_file_record(&mut transaction, file_id.as_str()).await;
        finish_read(transaction, result).await
    }

    /// Lists a Project's current `FileRecords` in normalized path order.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the query or stored row decoding fails.
    pub async fn list_file_records(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<FileRecord>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = async {
            let rows = sqlx::query(
                "SELECT id, project_id, relative_path, normalized_relative_path,
                    size_bytes, modified_at, content_hash, encoding, language,
                    source_status, included, exclusion_reason,
                    sensitive_findings_json, result_status, latest_successful_run_id,
                    latest_successful_task_id, scan_generation, created_at, updated_at
                 FROM file_records WHERE project_id = ?
                 ORDER BY normalized_relative_path ASC, id ASC",
            )
            .bind(project_id.as_str())
            .fetch_all(transaction.connection())
            .await
            .map_err(|_| PersistenceError::TransactionFailed)?;

            rows.into_iter()
                .map(|row| file_record_from_row(&row))
                .collect()
        }
        .await;
        finish_read(transaction, result).await
    }

    /// Inserts an immutable `ContextVersion`.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the row cannot be encoded or the
    /// transaction fails.
    pub async fn create_context_version(
        &self,
        context_version: &ContextVersion,
    ) -> Result<(), PersistenceError> {
        let row = ContextVersionRow::from(context_version);
        let mut transaction = self.database.begin_write().await?;
        let result = insert_context_version(&mut transaction, &row).await;
        finish_write(transaction, result).await
    }

    /// Loads a `ContextVersion` by ID.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the query or stored row decoding fails.
    pub async fn get_context_version(
        &self,
        context_version_id: &ContextVersionId,
    ) -> Result<Option<ContextVersion>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = load_context_version(&mut transaction, context_version_id.as_str()).await;
        finish_read(transaction, result).await
    }

    /// Creates a Run and all initial Tasks in one transaction.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when a task belongs to another Run, a
    /// foreign key or uniqueness constraint fails, or the transaction fails.
    pub async fn create_run_with_tasks(
        &self,
        run: &Run,
        metadata: RunRowMetadata,
        tasks: &[Task],
    ) -> Result<Run, PersistenceError> {
        if tasks.iter().any(|task| task.run_id != run.id) {
            return Err(PersistenceError::InvalidStoredState);
        }

        let row = RunRow::from_domain(run, metadata)?;
        let task_rows: Vec<TaskRow> = tasks.iter().map(TaskRow::from).collect();
        let mut transaction = self.database.begin_write().await?;
        let result = async {
            insert_run(&mut transaction, &row).await?;
            for task in &task_rows {
                insert_task(&mut transaction, task).await?;
            }

            let stats =
                recompute_run_stats_in_transaction(&mut transaction, run.id.as_str()).await?;
            update_run_stats(&mut transaction, run.id.as_str(), &stats).await?;

            let mut created = run.clone();
            created.stats = stats;
            Ok(created)
        }
        .await;
        finish_write(transaction, result).await
    }

    /// Loads a Run by ID.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the query or stored row decoding fails.
    pub async fn get_run(&self, run_id: &RunId) -> Result<Option<Run>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = load_run(&mut transaction, run_id.as_str()).await;
        finish_read(transaction, result).await
    }

    /// Lists `Run`s newest first for a Project.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the query or stored row decoding fails.
    pub async fn list_runs(&self, project_id: &ProjectId) -> Result<Vec<Run>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = async {
            let rows = sqlx::query(
                "SELECT id, project_id, status, context_version_id,
                    output_directory, snapshot_json, stats_json, created_at,
                    started_at, completed_at, interruption_reason
                 FROM runs WHERE project_id = ? ORDER BY created_at DESC, id DESC",
            )
            .bind(project_id.as_str())
            .fetch_all(transaction.connection())
            .await
            .map_err(|_| PersistenceError::TransactionFailed)?;

            rows.into_iter().map(|row| run_from_row(&row)).collect()
        }
        .await;
        finish_read(transaction, result).await
    }

    /// Recomputes Run statistics from the authoritative Task rows.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the Run is missing, a stored status is
    /// invalid, or the transaction fails.
    pub async fn recompute_run_stats(&self, run_id: &RunId) -> Result<RunStats, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = async {
            ensure_run_exists(&mut transaction, run_id.as_str()).await?;
            let stats =
                recompute_run_stats_in_transaction(&mut transaction, run_id.as_str()).await?;
            update_run_stats(&mut transaction, run_id.as_str(), &stats).await?;
            Ok(stats)
        }
        .await;
        finish_write(transaction, result).await
    }

    /// Claims the oldest queued Task for a Run atomically.
    ///
    /// The single conditional UPDATE prevents two workers from receiving the
    /// same Task. The selected state is validated through TASK-0002's state
    /// machine before the SQL is executed.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the Run is missing, the state cannot be
    /// serialized, or the transaction fails.
    pub async fn claim_next_task(
        &self,
        run_id: &RunId,
        now: &batch_code_analyzer_domain::Rfc3339Timestamp,
    ) -> Result<Option<Task>, PersistenceError> {
        let next_status =
            TaskStateMachine::transition(TaskStatus::Queued, TaskTransition::Claim)
                .map_err(|error| PersistenceError::StateTransition { code: error.code() })?;
        let queued = status_string(TaskStatus::Queued)?;
        let running = status_string(next_status)?;
        let mut transaction = self.database.begin_write().await?;
        let result = async {
            ensure_run_exists(&mut transaction, run_id.as_str()).await?;
            let id: Option<String> = sqlx::query_scalar(
                "UPDATE tasks SET status = ?, started_at = ?
                 WHERE id = (
                    SELECT tasks.id FROM tasks
                    WHERE tasks.run_id = ? AND tasks.status = ?
                    ORDER BY tasks.created_at ASC, tasks.id ASC LIMIT 1
                 ) RETURNING id",
            )
            .bind(running)
            .bind(now.as_str())
            .bind(run_id.as_str())
            .bind(queued)
            .fetch_optional(transaction.connection())
            .await
            .map_err(|_| PersistenceError::TransactionFailed)?;

            match id {
                Some(id) => load_task(&mut transaction, &id).await,
                None => Ok(None),
            }
        }
        .await;
        finish_write(transaction, result).await
    }

    /// Claims one specific queued Task atomically.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the Task is missing, its current state
    /// cannot be claimed, or the transaction fails.
    pub async fn claim_task(
        &self,
        task_id: &TaskId,
        now: &batch_code_analyzer_domain::Rfc3339Timestamp,
    ) -> Result<Task, PersistenceError> {
        let running = status_string(
            TaskStateMachine::transition(TaskStatus::Queued, TaskTransition::Claim)
                .map_err(|error| PersistenceError::StateTransition { code: error.code() })?,
        )?;
        let queued = status_string(TaskStatus::Queued)?;
        let mut transaction = self.database.begin_write().await?;
        let result = async {
            let id: Option<String> = sqlx::query_scalar(
                "UPDATE tasks SET status = ?, started_at = ?
                 WHERE id = ? AND status = ? RETURNING id",
            )
            .bind(running)
            .bind(now.as_str())
            .bind(task_id.as_str())
            .bind(queued)
            .fetch_optional(transaction.connection())
            .await
            .map_err(|_| PersistenceError::TransactionFailed)?;

            if let Some(id) = id {
                load_task(&mut transaction, &id)
                    .await?
                    .ok_or_else(|| task_not_found(task_id.as_str()))
            } else {
                let current = load_task(&mut transaction, task_id.as_str())
                    .await?
                    .ok_or_else(|| task_not_found(task_id.as_str()))?;
                let current_status = current.status;
                TaskStateMachine::transition(current_status, TaskTransition::Claim)
                    .map(|_| current)
                    .map_err(|error| PersistenceError::StateTransition { code: error.code() })
            }
        }
        .await;
        finish_write(transaction, result).await
    }

    /// Appends an Attempt. There is intentionally no update/delete API.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the sequence is invalid, the Task is
    /// missing, a duplicate sequence is supplied, or the transaction fails.
    pub async fn append_attempt(
        &self,
        attempt: &Attempt,
        metadata: AttemptRowMetadata,
    ) -> Result<(), PersistenceError> {
        if attempt.sequence == 0 {
            return Err(PersistenceError::InvalidStoredState);
        }
        let row = AttemptRow::from_domain(attempt, metadata)?;
        let mut transaction = self.database.begin_write().await?;
        let result = insert_attempt(&mut transaction, &row).await;
        finish_write(transaction, result).await
    }

    /// Loads an Attempt by ID.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the query or stored row decoding fails.
    pub async fn get_attempt(
        &self,
        attempt_id: &AttemptId,
    ) -> Result<Option<Attempt>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = load_attempt(&mut transaction, attempt_id.as_str()).await;
        finish_read(transaction, result).await
    }

    /// Lists all Attempts for a Task in append order.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the query or stored row decoding fails.
    pub async fn list_attempts(&self, task_id: &TaskId) -> Result<Vec<Attempt>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = async {
            let rows = sqlx::query(
                "SELECT id, task_id, sequence, api_profile_id,
                    api_profile_name_snapshot, actual_model, status, created_at,
                    request_started_at, request_dispatched_at, finished_at,
                    duration_ms, http_status, input_tokens, output_tokens,
                    total_tokens, retry_reason, error_code,
                    sanitized_error_message, error_retryable, error_sanitized,
                    response_id
                 FROM attempts WHERE task_id = ? ORDER BY sequence ASC",
            )
            .bind(task_id.as_str())
            .fetch_all(transaction.connection())
            .await
            .map_err(|_| PersistenceError::TransactionFailed)?;

            rows.into_iter().map(|row| attempt_from_row(&row)).collect()
        }
        .await;
        finish_read(transaction, result).await
    }

    /// Returns Runs that need crash recovery handling.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the query or stored row decoding fails.
    pub async fn unfinished_runs(&self) -> Result<Vec<Run>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = load_runs_by_status(
            &mut transaction,
            &[
                RunStatus::Running,
                RunStatus::Pausing,
                RunStatus::Cancelling,
            ],
        )
        .await;
        finish_read(transaction, result).await
    }

    /// Returns Tasks currently in flight and therefore needing recovery.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the query or stored row decoding fails.
    pub async fn unfinished_tasks(&self, run_id: &RunId) -> Result<Vec<Task>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result =
            load_tasks_by_status(&mut transaction, run_id.as_str(), &[TaskStatus::Running]).await;
        finish_read(transaction, result).await
    }

    /// Returns Attempts whose provider outcome is not yet known.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the query or stored row decoding fails.
    pub async fn unfinished_attempts(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<Attempt>, PersistenceError> {
        let mut transaction = self.database.begin_write().await?;
        let result = async {
            let created = status_string(batch_code_analyzer_domain::AttemptStatus::Created)?;
            let dispatched = status_string(batch_code_analyzer_domain::AttemptStatus::Dispatched)?;
            let rows = sqlx::query(
                "SELECT id, task_id, sequence, api_profile_id,
                    api_profile_name_snapshot, actual_model, status, created_at,
                    request_started_at, request_dispatched_at, finished_at,
                    duration_ms, http_status, input_tokens, output_tokens,
                    total_tokens, retry_reason, error_code,
                    sanitized_error_message, error_retryable, error_sanitized,
                    response_id
                 FROM attempts WHERE task_id = ? AND status IN (?, ?)
                 ORDER BY sequence ASC",
            )
            .bind(task_id.as_str())
            .bind(created)
            .bind(dispatched)
            .fetch_all(transaction.connection())
            .await
            .map_err(|_| PersistenceError::TransactionFailed)?;
            rows.into_iter().map(|row| attempt_from_row(&row)).collect()
        }
        .await;
        finish_read(transaction, result).await
    }
}

async fn finish_read<T>(
    transaction: WriteTransaction<'_>,
    result: Result<T, PersistenceError>,
) -> Result<T, PersistenceError> {
    let rollback = transaction.rollback().await;
    match result {
        Err(error) => Err(error),
        Ok(value) => rollback.map(|()| value),
    }
}

async fn finish_write<T>(
    transaction: WriteTransaction<'_>,
    result: Result<T, PersistenceError>,
) -> Result<T, PersistenceError> {
    match result {
        Ok(value) => transaction.commit().await.map(|()| value),
        Err(error) => {
            let _ = transaction.rollback().await;
            Err(error)
        }
    }
}

async fn insert_project(
    transaction: &mut WriteTransaction<'_>,
    row: &ProjectRow,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO projects (
            id, schema_version, name, source_directory, canonical_source_directory,
            path_status, default_prompt, default_model, context_model, output_root,
            filter_rules_json, execution_defaults_json, api_routing_json,
            current_context_version_id, context_enabled, context_status,
            created_at, updated_at, last_opened_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&row.id)
    .bind(row.schema_version)
    .bind(&row.name)
    .bind(&row.source_directory)
    .bind(&row.canonical_source_directory)
    .bind(&row.path_status)
    .bind(&row.default_prompt)
    .bind(&row.default_model)
    .bind(&row.context_model)
    .bind(&row.output_root)
    .bind(&row.filter_rules_json)
    .bind(&row.execution_defaults_json)
    .bind(&row.api_routing_json)
    .bind(&row.current_context_version_id)
    .bind(row.context_enabled)
    .bind(&row.context_status)
    .bind(&row.created_at)
    .bind(&row.updated_at)
    .bind(&row.last_opened_at)
    .execute(transaction.connection())
    .await
    .map_err(|error| map_project_write_error(&error))?;
    Ok(())
}

async fn insert_file_record(
    transaction: &mut WriteTransaction<'_>,
    row: &FileRecordRow,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO file_records (
            id, project_id, relative_path, normalized_relative_path, size_bytes,
            modified_at, content_hash, encoding, language, source_status, included,
            exclusion_reason, sensitive_findings_json, result_status,
            latest_successful_run_id, latest_successful_task_id, scan_generation,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&row.id)
    .bind(&row.project_id)
    .bind(&row.relative_path)
    .bind(&row.normalized_relative_path)
    .bind(row.size_bytes)
    .bind(&row.modified_at)
    .bind(&row.content_hash)
    .bind(&row.encoding)
    .bind(&row.language)
    .bind(&row.source_status)
    .bind(row.included)
    .bind(&row.exclusion_reason)
    .bind(&row.sensitive_findings_json)
    .bind(&row.result_status)
    .bind(&row.latest_successful_run_id)
    .bind(&row.latest_successful_task_id)
    .bind(row.scan_generation)
    .bind(&row.created_at)
    .bind(&row.updated_at)
    .execute(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    Ok(())
}

async fn insert_context_version(
    transaction: &mut WriteTransaction<'_>,
    row: &ContextVersionRow,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO context_versions (
            id, project_id, status, model, source_files_json, summary,
            summary_hash, manually_edited, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&row.id)
    .bind(&row.project_id)
    .bind(&row.status)
    .bind(&row.model)
    .bind(&row.source_files_json)
    .bind(&row.summary)
    .bind(&row.summary_hash)
    .bind(row.manually_edited)
    .bind(&row.created_at)
    .execute(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    Ok(())
}

async fn insert_run(
    transaction: &mut WriteTransaction<'_>,
    row: &RunRow,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO runs (
            id, project_id, status, context_version_id, output_directory,
            snapshot_json, stats_json, created_at, started_at, completed_at,
            interruption_reason
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&row.id)
    .bind(&row.project_id)
    .bind(&row.status)
    .bind(&row.context_version_id)
    .bind(&row.output_directory)
    .bind(&row.snapshot_json)
    .bind(&row.stats_json)
    .bind(&row.created_at)
    .bind(&row.started_at)
    .bind(&row.completed_at)
    .bind(&row.interruption_reason)
    .execute(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    Ok(())
}

async fn insert_task(
    transaction: &mut WriteTransaction<'_>,
    row: &TaskRow,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO tasks (
            id, run_id, file_id, relative_path, file_snapshot_json, prompt_snapshot,
            prompt_hash, prompt_source, model_snapshot, model_source,
            context_version_id, status, current_result_path, latest_attempt_id,
            parent_task_id, result_version, created_at, started_at, completed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&row.id)
    .bind(&row.run_id)
    .bind(&row.file_id)
    .bind(&row.relative_path)
    .bind(&row.file_snapshot_json)
    .bind(&row.prompt_snapshot)
    .bind(&row.prompt_hash)
    .bind(&row.prompt_source)
    .bind(&row.model_snapshot)
    .bind(&row.model_source)
    .bind(&row.context_version_id)
    .bind(&row.status)
    .bind(&row.current_result_path)
    .bind(&row.latest_attempt_id)
    .bind(&row.parent_task_id)
    .bind(row.result_version)
    .bind(&row.created_at)
    .bind(&row.started_at)
    .bind(&row.completed_at)
    .execute(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    Ok(())
}

async fn insert_attempt(
    transaction: &mut WriteTransaction<'_>,
    row: &AttemptRow,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO attempts (
            id, task_id, sequence, api_profile_id, api_profile_name_snapshot,
            actual_model, status, created_at, request_started_at,
            request_dispatched_at, finished_at, duration_ms, http_status,
            input_tokens, output_tokens, total_tokens, retry_reason, error_code,
            sanitized_error_message, error_retryable, error_sanitized, response_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&row.id)
    .bind(&row.task_id)
    .bind(row.sequence)
    .bind(&row.api_profile_id)
    .bind(&row.api_profile_name_snapshot)
    .bind(&row.actual_model)
    .bind(&row.status)
    .bind(&row.created_at)
    .bind(&row.request_started_at)
    .bind(&row.request_dispatched_at)
    .bind(&row.finished_at)
    .bind(row.duration_ms)
    .bind(row.http_status)
    .bind(row.input_tokens)
    .bind(row.output_tokens)
    .bind(row.total_tokens)
    .bind(&row.retry_reason)
    .bind(&row.error_code)
    .bind(&row.sanitized_error_message)
    .bind(row.error_retryable)
    .bind(row.error_sanitized)
    .bind(&row.response_id)
    .execute(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    Ok(())
}

async fn load_project(
    transaction: &mut WriteTransaction<'_>,
    id: &str,
) -> Result<Option<Project>, PersistenceError> {
    let row = sqlx::query(
        "SELECT id, schema_version, name, source_directory,
            canonical_source_directory, path_status, default_prompt, default_model,
            context_model, output_root, filter_rules_json, execution_defaults_json,
            api_routing_json, current_context_version_id, context_enabled,
            context_status, created_at, updated_at, last_opened_at
         FROM projects WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    row.map(|row| project_from_row(&row)).transpose()
}

async fn load_file_record(
    transaction: &mut WriteTransaction<'_>,
    id: &str,
) -> Result<Option<FileRecord>, PersistenceError> {
    let row = sqlx::query(
        "SELECT id, project_id, relative_path, normalized_relative_path,
            size_bytes, modified_at, content_hash, encoding, language, source_status,
            included, exclusion_reason, sensitive_findings_json, result_status,
            latest_successful_run_id, latest_successful_task_id, scan_generation,
            created_at, updated_at
         FROM file_records WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    row.map(|row| file_record_from_row(&row)).transpose()
}

async fn load_context_version(
    transaction: &mut WriteTransaction<'_>,
    id: &str,
) -> Result<Option<ContextVersion>, PersistenceError> {
    let row = sqlx::query(
        "SELECT id, project_id, status, model, source_files_json, summary,
            summary_hash, manually_edited, created_at
         FROM context_versions WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    row.map(|row| context_version_from_row(&row)).transpose()
}

async fn load_run(
    transaction: &mut WriteTransaction<'_>,
    id: &str,
) -> Result<Option<Run>, PersistenceError> {
    let row = sqlx::query(
        "SELECT id, project_id, status, context_version_id, output_directory,
            snapshot_json, stats_json, created_at, started_at, completed_at,
            interruption_reason
         FROM runs WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    row.map(|row| run_from_row(&row)).transpose()
}

async fn load_task(
    transaction: &mut WriteTransaction<'_>,
    id: &str,
) -> Result<Option<Task>, PersistenceError> {
    let row = sqlx::query(
        "SELECT id, run_id, file_id, relative_path, file_snapshot_json,
            prompt_snapshot, prompt_hash, prompt_source, model_snapshot,
            model_source, context_version_id, status, current_result_path,
            latest_attempt_id, parent_task_id, result_version, created_at,
            started_at, completed_at
         FROM tasks WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    row.map(|row| task_from_row(&row)).transpose()
}

async fn load_attempt(
    transaction: &mut WriteTransaction<'_>,
    id: &str,
) -> Result<Option<Attempt>, PersistenceError> {
    let row = sqlx::query(
        "SELECT id, task_id, sequence, api_profile_id, api_profile_name_snapshot,
            actual_model, status, created_at, request_started_at,
            request_dispatched_at, finished_at, duration_ms, http_status,
            input_tokens, output_tokens, total_tokens, retry_reason, error_code,
            sanitized_error_message, error_retryable, error_sanitized, response_id
         FROM attempts WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    row.map(|row| attempt_from_row(&row)).transpose()
}

async fn load_runs_by_status(
    transaction: &mut WriteTransaction<'_>,
    statuses: &[RunStatus],
) -> Result<Vec<Run>, PersistenceError> {
    let values = statuses
        .iter()
        .copied()
        .map(status_string)
        .collect::<Result<Vec<_>, _>>()?;
    let rows = sqlx::query(
        "SELECT id, project_id, status, context_version_id, output_directory,
            snapshot_json, stats_json, created_at, started_at, completed_at,
            interruption_reason
         FROM runs WHERE status IN (?, ?, ?) ORDER BY created_at ASC, id ASC",
    )
    .bind(values.first())
    .bind(values.get(1))
    .bind(values.get(2))
    .fetch_all(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    rows.into_iter().map(|row| run_from_row(&row)).collect()
}

async fn load_tasks_by_status(
    transaction: &mut WriteTransaction<'_>,
    run_id: &str,
    statuses: &[TaskStatus],
) -> Result<Vec<Task>, PersistenceError> {
    let values = statuses
        .iter()
        .copied()
        .map(status_string)
        .collect::<Result<Vec<_>, _>>()?;
    let rows = sqlx::query(
        "SELECT id, run_id, file_id, relative_path, file_snapshot_json,
            prompt_snapshot, prompt_hash, prompt_source, model_snapshot,
            model_source, context_version_id, status, current_result_path,
            latest_attempt_id, parent_task_id, result_version, created_at,
            started_at, completed_at
         FROM tasks WHERE run_id = ? AND status IN (?)
         ORDER BY created_at ASC, id ASC",
    )
    .bind(run_id)
    .bind(values.first())
    .fetch_all(transaction.connection())
    .await
    .map_err(|_| PersistenceError::TransactionFailed)?;
    rows.into_iter().map(|row| task_from_row(&row)).collect()
}

async fn ensure_run_exists(
    transaction: &mut WriteTransaction<'_>,
    id: &str,
) -> Result<(), PersistenceError> {
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM runs WHERE id = ?")
        .bind(id)
        .fetch_optional(transaction.connection())
        .await
        .map_err(|_| PersistenceError::TransactionFailed)?;
    exists.map(|_| ()).ok_or_else(|| run_not_found(id))
}

async fn recompute_run_stats_in_transaction(
    transaction: &mut WriteTransaction<'_>,
    run_id: &str,
) -> Result<RunStats, PersistenceError> {
    let rows =
        sqlx::query("SELECT status, COUNT(*) AS count FROM tasks WHERE run_id = ? GROUP BY status")
            .bind(run_id)
            .fetch_all(transaction.connection())
            .await
            .map_err(|_| PersistenceError::TransactionFailed)?;
    let mut stats = RunStats::default();
    for row in rows {
        let task_status: String = row
            .try_get("status")
            .map_err(|_| PersistenceError::InvalidStoredState)?;
        let count: i64 = row
            .try_get("count")
            .map_err(|_| PersistenceError::InvalidStoredState)?;
        let count = u32::try_from(count).map_err(|_| PersistenceError::InvalidStoredState)?;
        stats.total = stats
            .total
            .checked_add(count)
            .ok_or(PersistenceError::InvalidStoredState)?;
        match decode_status::<TaskStatus>(&task_status)? {
            TaskStatus::Pending => stats.pending = count,
            TaskStatus::Queued => stats.queued = count,
            TaskStatus::Running => stats.running = count,
            TaskStatus::Succeeded => stats.succeeded = count,
            TaskStatus::Failed => stats.failed = count,
            TaskStatus::Cancelled => stats.cancelled = count,
            TaskStatus::Interrupted => stats.interrupted = count,
            TaskStatus::SourceChanged => stats.source_changed = count,
        }
    }
    Ok(stats)
}

async fn update_run_stats(
    transaction: &mut WriteTransaction<'_>,
    run_id: &str,
    stats: &RunStats,
) -> Result<(), PersistenceError> {
    let stats_json = encode(stats)?;
    sqlx::query("UPDATE runs SET stats_json = ? WHERE id = ?")
        .bind(stats_json)
        .bind(run_id)
        .execute(transaction.connection())
        .await
        .map_err(|_| PersistenceError::TransactionFailed)?;
    Ok(())
}

fn project_from_row(row: &SqliteRow) -> Result<Project, PersistenceError> {
    ProjectRow {
        id: get(row, "id")?,
        schema_version: get(row, "schema_version")?,
        name: get(row, "name")?,
        source_directory: get(row, "source_directory")?,
        canonical_source_directory: get(row, "canonical_source_directory")?,
        path_status: get(row, "path_status")?,
        default_prompt: get(row, "default_prompt")?,
        default_model: get(row, "default_model")?,
        context_model: get(row, "context_model")?,
        output_root: get(row, "output_root")?,
        filter_rules_json: get(row, "filter_rules_json")?,
        execution_defaults_json: get(row, "execution_defaults_json")?,
        api_routing_json: get(row, "api_routing_json")?,
        current_context_version_id: get(row, "current_context_version_id")?,
        context_enabled: get(row, "context_enabled")?,
        context_status: get(row, "context_status")?,
        created_at: get(row, "created_at")?,
        updated_at: get(row, "updated_at")?,
        last_opened_at: get(row, "last_opened_at")?,
    }
    .try_into()
}

fn file_record_from_row(row: &SqliteRow) -> Result<FileRecord, PersistenceError> {
    FileRecordRow {
        id: get(row, "id")?,
        project_id: get(row, "project_id")?,
        relative_path: get(row, "relative_path")?,
        normalized_relative_path: get(row, "normalized_relative_path")?,
        size_bytes: get(row, "size_bytes")?,
        modified_at: get(row, "modified_at")?,
        content_hash: get(row, "content_hash")?,
        encoding: get(row, "encoding")?,
        language: get(row, "language")?,
        source_status: get(row, "source_status")?,
        included: get(row, "included")?,
        exclusion_reason: get(row, "exclusion_reason")?,
        sensitive_findings_json: get(row, "sensitive_findings_json")?,
        result_status: get(row, "result_status")?,
        latest_successful_run_id: get(row, "latest_successful_run_id")?,
        latest_successful_task_id: get(row, "latest_successful_task_id")?,
        scan_generation: get(row, "scan_generation")?,
        created_at: get(row, "created_at")?,
        updated_at: get(row, "updated_at")?,
    }
    .try_into()
}

fn context_version_from_row(row: &SqliteRow) -> Result<ContextVersion, PersistenceError> {
    ContextVersionRow {
        id: get(row, "id")?,
        project_id: get(row, "project_id")?,
        status: get(row, "status")?,
        model: get(row, "model")?,
        source_files_json: get(row, "source_files_json")?,
        summary: get(row, "summary")?,
        summary_hash: get(row, "summary_hash")?,
        manually_edited: get(row, "manually_edited")?,
        created_at: get(row, "created_at")?,
    }
    .try_into()
}

fn run_from_row(row: &SqliteRow) -> Result<Run, PersistenceError> {
    RunRow {
        id: get(row, "id")?,
        project_id: get(row, "project_id")?,
        status: get(row, "status")?,
        context_version_id: get(row, "context_version_id")?,
        output_directory: get(row, "output_directory")?,
        snapshot_json: get(row, "snapshot_json")?,
        stats_json: get(row, "stats_json")?,
        created_at: get(row, "created_at")?,
        started_at: get(row, "started_at")?,
        completed_at: get(row, "completed_at")?,
        interruption_reason: get(row, "interruption_reason")?,
    }
    .try_into()
}

fn task_from_row(row: &SqliteRow) -> Result<Task, PersistenceError> {
    TaskRow {
        id: get(row, "id")?,
        run_id: get(row, "run_id")?,
        file_id: get(row, "file_id")?,
        relative_path: get(row, "relative_path")?,
        file_snapshot_json: get(row, "file_snapshot_json")?,
        prompt_snapshot: get(row, "prompt_snapshot")?,
        prompt_hash: get(row, "prompt_hash")?,
        prompt_source: get(row, "prompt_source")?,
        model_snapshot: get(row, "model_snapshot")?,
        model_source: get(row, "model_source")?,
        context_version_id: get(row, "context_version_id")?,
        status: get(row, "status")?,
        current_result_path: get(row, "current_result_path")?,
        latest_attempt_id: get(row, "latest_attempt_id")?,
        parent_task_id: get(row, "parent_task_id")?,
        result_version: get(row, "result_version")?,
        created_at: get(row, "created_at")?,
        started_at: get(row, "started_at")?,
        completed_at: get(row, "completed_at")?,
    }
    .try_into()
}

fn attempt_from_row(row: &SqliteRow) -> Result<Attempt, PersistenceError> {
    AttemptRow {
        id: get(row, "id")?,
        task_id: get(row, "task_id")?,
        sequence: get(row, "sequence")?,
        api_profile_id: get(row, "api_profile_id")?,
        api_profile_name_snapshot: get(row, "api_profile_name_snapshot")?,
        actual_model: get(row, "actual_model")?,
        status: get(row, "status")?,
        created_at: get(row, "created_at")?,
        request_started_at: get(row, "request_started_at")?,
        request_dispatched_at: get(row, "request_dispatched_at")?,
        finished_at: get(row, "finished_at")?,
        duration_ms: get(row, "duration_ms")?,
        http_status: get(row, "http_status")?,
        input_tokens: get(row, "input_tokens")?,
        output_tokens: get(row, "output_tokens")?,
        total_tokens: get(row, "total_tokens")?,
        retry_reason: get(row, "retry_reason")?,
        error_code: get(row, "error_code")?,
        sanitized_error_message: get(row, "sanitized_error_message")?,
        error_retryable: get(row, "error_retryable")?,
        error_sanitized: get(row, "error_sanitized")?,
        response_id: get(row, "response_id")?,
    }
    .try_into()
}

fn get<T>(row: &SqliteRow, column: &str) -> Result<T, PersistenceError>
where
    for<'r> T: sqlx::Decode<'r, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite>,
{
    row.try_get(column)
        .map_err(|_| PersistenceError::InvalidStoredState)
}

fn encode<T: Serialize>(value: &T) -> Result<String, PersistenceError> {
    serde_json::to_string(value).map_err(|_| PersistenceError::InvalidStoredState)
}

fn decode_status<T: DeserializeOwned>(value: &str) -> Result<T, PersistenceError> {
    serde_json::from_value(Value::String(value.to_owned()))
        .map_err(|_| PersistenceError::InvalidStoredState)
}

fn status_string<T: Serialize>(status: T) -> Result<String, PersistenceError> {
    match serde_json::to_value(status).map_err(|_| PersistenceError::InvalidStoredState)? {
        Value::String(value) => Ok(value),
        _ => Err(PersistenceError::InvalidStoredState),
    }
}

fn map_project_write_error(error: &sqlx::Error) -> PersistenceError {
    if let sqlx::Error::Database(database_error) = error {
        if database_error.message().contains("UNIQUE") {
            return PersistenceError::StateTransition {
                code: "project_path_duplicate",
            };
        }
    }
    PersistenceError::TransactionFailed
}

fn project_not_found(id: &str) -> PersistenceError {
    PersistenceError::RecordNotFound {
        kind: "project",
        id: id.to_owned(),
    }
}

fn run_not_found(id: &str) -> PersistenceError {
    PersistenceError::RecordNotFound {
        kind: "run",
        id: id.to_owned(),
    }
}

fn task_not_found(id: &str) -> PersistenceError {
    PersistenceError::RecordNotFound {
        kind: "task",
        id: id.to_owned(),
    }
}
