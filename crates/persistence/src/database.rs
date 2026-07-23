use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use batch_code_analyzer_domain::{
    RunId, RunStateMachine, RunStatus, RunTransition, TaskId, TaskStateMachine, TaskStatus,
    TaskTransition,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    Sqlite, SqliteConnection, SqlitePool, Transaction,
};
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::PersistenceError;

pub const LATEST_SCHEMA_VERSION: u32 = 2;
const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseHealth {
    Ready { schema_version: u32 },
    MigrationFailed { schema_version: u32 },
    Unavailable,
}

#[derive(Debug)]
pub struct RecoveryDatabase {
    path: PathBuf,
    health: DatabaseHealth,
    cause: PersistenceError,
}

impl RecoveryDatabase {
    #[must_use]
    pub fn health(&self) -> DatabaseHealth {
        self.health
    }

    #[must_use]
    pub fn cause(&self) -> &PersistenceError {
        &self.cause
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Opens the failed database without write access for a future recovery UI.
    ///
    /// # Errors
    ///
    /// Returns `persistence_database_unavailable` when the database cannot be
    /// opened for read-only recovery.
    pub async fn open_read_only(&self) -> Result<ReadOnlyDatabase, PersistenceError> {
        let pool = connect(read_only_sqlite_options(&self.path), 1).await?;
        let schema_version = applied_schema_version(&pool).await?;

        Ok(ReadOnlyDatabase {
            _pool: pool,
            schema_version: u32::try_from(schema_version)
                .map_err(|_| PersistenceError::InvalidStoredState)?,
        })
    }
}

#[derive(Debug)]
pub struct ReadOnlyDatabase {
    _pool: SqlitePool,
    schema_version: u32,
}

impl ReadOnlyDatabase {
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
}

#[derive(Debug)]
pub enum DatabaseStartup {
    Ready(Database),
    Recovery(RecoveryDatabase),
}

impl DatabaseStartup {
    #[must_use]
    pub fn health(&self) -> DatabaseHealth {
        match self {
            Self::Ready(database) => database.health(),
            Self::Recovery(recovery) => recovery.health(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Database {
    pool: SqlitePool,
    schema_version: u32,
    write_gate: Arc<Mutex<()>>,
}

impl Database {
    /// Opens, checks, and migrates the persistent `SQLite` database.
    ///
    /// # Errors
    ///
    /// Returns a stable persistence error if the database cannot be opened,
    /// is newer than this application, or cannot be migrated.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
        let path = path.as_ref();
        create_parent_directory(path)?;
        backup_existing_database(path)?;

        let pool = connect(sqlite_options(path, true), 5).await?;
        Self::from_pool(pool).await
    }

    /// Opens the database and exposes a recovery entry point instead of
    /// preventing the desktop shell from starting after migration failure.
    pub async fn open_for_startup(path: impl AsRef<Path>) -> DatabaseStartup {
        let path = path.as_ref().to_path_buf();

        match Self::open(&path).await {
            Ok(database) => DatabaseStartup::Ready(database),
            Err(cause) => {
                let health = match cause {
                    PersistenceError::MigrationFailed => {
                        DatabaseHealth::MigrationFailed { schema_version: 0 }
                    }
                    PersistenceError::SchemaTooNew { detected, .. } => {
                        DatabaseHealth::MigrationFailed {
                            schema_version: detected,
                        }
                    }
                    _ => DatabaseHealth::Unavailable,
                };

                DatabaseStartup::Recovery(RecoveryDatabase {
                    path,
                    health,
                    cause,
                })
            }
        }
    }

    /// Creates an isolated in-memory database for integration tests.
    ///
    /// A single connection is intentional: `SQLite` in-memory databases are
    /// scoped to a connection, while this helper must expose one shared schema.
    ///
    /// # Errors
    ///
    /// Returns a stable persistence error if the temporary database cannot be
    /// initialized or migrated.
    pub async fn open_in_memory() -> Result<Self, PersistenceError> {
        let options = SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(true)
            .busy_timeout(DEFAULT_BUSY_TIMEOUT);
        let pool = connect(options, 1).await?;

        Self::from_pool(pool).await
    }

    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub const fn health(&self) -> DatabaseHealth {
        DatabaseHealth::Ready {
            schema_version: self.schema_version,
        }
    }

    /// Begins a write transaction for persistence repositories.
    ///
    /// # Errors
    ///
    /// Returns `persistence_transaction_failed` if `SQLite` cannot begin the
    /// transaction.
    pub async fn begin_write(&self) -> Result<WriteTransaction<'_>, PersistenceError> {
        let write_guard = self.write_gate.clone().lock_owned().await;
        let transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| PersistenceError::TransactionFailed)?;

        Ok(WriteTransaction {
            transaction,
            _write_guard: write_guard,
        })
    }

    /// Applies a documented Run transition atomically after loading the
    /// current persisted status. Callers cannot provide a replacement status.
    ///
    /// # Errors
    ///
    /// Returns a state-machine error code for an invalid transition, or a
    /// stable persistence error for storage failures.
    pub async fn transition_run_status(
        &self,
        run_id: &RunId,
        event: RunTransition,
    ) -> Result<RunStatus, PersistenceError> {
        let mut transaction = self.begin_write().await?;
        let current = load_status::<RunStatus>(
            "SELECT status FROM runs WHERE id = ?",
            run_id.as_str(),
            &mut transaction.transaction,
            "run",
        )
        .await?;
        let next = RunStateMachine::transition(current, event)
            .map_err(|error| PersistenceError::StateTransition { code: error.code() })?;

        sqlx::query("UPDATE runs SET status = ? WHERE id = ?")
            .bind(serialize_status(next)?)
            .bind(run_id.as_str())
            .execute(&mut *transaction.transaction)
            .await
            .map_err(|_| PersistenceError::TransactionFailed)?;
        transaction.commit().await?;

        Ok(next)
    }

    /// Applies a documented Task transition atomically after loading the
    /// current persisted status. Callers cannot provide a replacement status.
    ///
    /// # Errors
    ///
    /// Returns a state-machine error code for an invalid transition, or a
    /// stable persistence error for storage failures.
    pub async fn transition_task_status(
        &self,
        task_id: &TaskId,
        event: TaskTransition,
    ) -> Result<TaskStatus, PersistenceError> {
        let mut transaction = self.begin_write().await?;
        let current = load_status::<TaskStatus>(
            "SELECT status FROM tasks WHERE id = ?",
            task_id.as_str(),
            &mut transaction.transaction,
            "task",
        )
        .await?;
        let next = TaskStateMachine::transition(current, event)
            .map_err(|error| PersistenceError::StateTransition { code: error.code() })?;

        sqlx::query("UPDATE tasks SET status = ? WHERE id = ?")
            .bind(serialize_status(next)?)
            .bind(task_id.as_str())
            .execute(&mut *transaction.transaction)
            .await
            .map_err(|_| PersistenceError::TransactionFailed)?;
        transaction.commit().await?;

        Ok(next)
    }

    async fn from_pool(pool: SqlitePool) -> Result<Self, PersistenceError> {
        let schema_version = apply_migrations(&pool).await?;

        Ok(Self {
            pool,
            schema_version,
            write_gate: Arc::new(Mutex::new(())),
        })
    }
}

pub struct WriteTransaction<'database> {
    transaction: Transaction<'database, Sqlite>,
    _write_guard: OwnedMutexGuard<()>,
}

impl WriteTransaction<'_> {
    /// Commits the transaction after all repository writes have succeeded.
    ///
    /// # Errors
    ///
    /// Returns `persistence_transaction_failed` if `SQLite` rejects the commit.
    pub async fn commit(self) -> Result<(), PersistenceError> {
        self.transaction
            .commit()
            .await
            .map_err(|_| PersistenceError::TransactionFailed)
    }

    /// Rolls back the transaction when a repository operation cannot finish.
    ///
    /// # Errors
    ///
    /// Returns `persistence_transaction_failed` if `SQLite` rejects the rollback.
    pub async fn rollback(self) -> Result<(), PersistenceError> {
        self.transaction
            .rollback()
            .await
            .map_err(|_| PersistenceError::TransactionFailed)
    }

    #[must_use]
    pub fn connection(&mut self) -> &mut SqliteConnection {
        &mut self.transaction
    }
}

fn sqlite_options(path: &Path, create_if_missing: bool) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(create_if_missing)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(DEFAULT_BUSY_TIMEOUT)
}

fn read_only_sqlite_options(path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .foreign_keys(true)
        .busy_timeout(DEFAULT_BUSY_TIMEOUT)
}

async fn connect(
    options: SqliteConnectOptions,
    max_connections: u32,
) -> Result<SqlitePool, PersistenceError> {
    SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await
        .map_err(|_| PersistenceError::DatabaseUnavailable)
}

async fn apply_migrations(pool: &SqlitePool) -> Result<u32, PersistenceError> {
    let applied_version = applied_schema_version(pool).await?;
    let supported_version = i64::from(LATEST_SCHEMA_VERSION);

    if applied_version > supported_version {
        return Err(PersistenceError::SchemaTooNew {
            detected: u32::try_from(applied_version).unwrap_or(u32::MAX),
            supported: LATEST_SCHEMA_VERSION,
        });
    }

    Migrator::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations"))
        .await
        .map_err(|_| PersistenceError::MigrationFailed)?
        .run(pool)
        .await
        .map_err(|_| PersistenceError::MigrationFailed)?;

    let migrated_version = applied_schema_version(pool).await?;
    if migrated_version != supported_version {
        return Err(PersistenceError::MigrationFailed);
    }

    Ok(LATEST_SCHEMA_VERSION)
}

async fn applied_schema_version(pool: &SqlitePool) -> Result<i64, PersistenceError> {
    let migrations_table: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await
    .map_err(|_| PersistenceError::DatabaseUnavailable)?;

    if migrations_table.is_none() {
        return Ok(0);
    }

    sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .map(|version: Option<i64>| version.unwrap_or(0))
        .map_err(|_| PersistenceError::DatabaseUnavailable)
}

async fn load_status<T>(
    statement: &str,
    id: &str,
    transaction: &mut Transaction<'_, Sqlite>,
    kind: &'static str,
) -> Result<T, PersistenceError>
where
    T: DeserializeOwned,
{
    let value: Option<String> = sqlx::query_scalar(statement)
        .bind(id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| PersistenceError::TransactionFailed)?;
    let value = value.ok_or_else(|| PersistenceError::RecordNotFound {
        kind,
        id: id.to_owned(),
    })?;

    serde_json::from_value(Value::String(value)).map_err(|_| PersistenceError::InvalidStoredState)
}

fn serialize_status<T>(status: T) -> Result<String, PersistenceError>
where
    T: Serialize,
{
    match serde_json::to_value(status).map_err(|_| PersistenceError::InvalidStoredState)? {
        Value::String(value) => Ok(value),
        _ => Err(PersistenceError::InvalidStoredState),
    }
}

fn create_parent_directory(path: &Path) -> Result<(), PersistenceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| PersistenceError::DatabaseUnavailable)?;
    }

    Ok(())
}

fn backup_existing_database(path: &Path) -> Result<(), PersistenceError> {
    if path.is_file() {
        let backup_path = path.with_extension("bak");
        fs::copy(path, &backup_path).map_err(|_| PersistenceError::DatabaseUnavailable)?;

        // A WAL database can hold committed pages outside the main database
        // file. Preserve its sidecars under the backup file's basename too.
        for suffix in ["-wal", "-shm"] {
            let source = database_sidecar_path(path, suffix)?;
            if source.is_file() {
                fs::copy(source, database_sidecar_path(&backup_path, suffix)?)
                    .map_err(|_| PersistenceError::DatabaseUnavailable)?;
            }
        }
    }

    Ok(())
}

fn database_sidecar_path(path: &Path, suffix: &str) -> Result<PathBuf, PersistenceError> {
    let mut file_name = path
        .file_name()
        .ok_or(PersistenceError::DatabaseUnavailable)?
        .to_os_string();
    file_name.push(suffix);

    Ok(path.with_file_name(file_name))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use super::{
        apply_migrations, Database, DatabaseHealth, DatabaseStartup, PersistenceError,
        LATEST_SCHEMA_VERSION,
    };
    use batch_code_analyzer_domain::{RunId, RunStatus, RunTransition};
    use sqlx::{query, query_scalar};
    use tokio::sync::oneshot;

    const TIMESTAMP: &str = "2026-07-17T10:00:00+08:00";

    #[tokio::test]
    async fn empty_database_migrates_to_the_latest_schema() {
        let database = Database::open_in_memory()
            .await
            .expect("temporary database should initialize");
        let table_count: i64 = query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN (
                'projects', 'api_profiles', 'file_records', 'context_versions',
                'runs', 'tasks', 'attempts', 'prompt_library', 'app_settings',
                'encrypted_secrets', 'secret_store_metadata'
            )",
        )
        .fetch_one(&database.pool)
        .await
        .expect("core tables should exist");

        assert_eq!(database.schema_version(), LATEST_SCHEMA_VERSION);
        assert_eq!(table_count, 11);
    }

    #[tokio::test]
    async fn repeated_migration_is_idempotent() {
        let database = Database::open_in_memory()
            .await
            .expect("temporary database should initialize");

        let version = apply_migrations(&database.pool)
            .await
            .expect("second migration should succeed");
        let migration_count: i64 = query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&database.pool)
            .await
            .expect("migration history should be readable");

        assert_eq!(version, LATEST_SCHEMA_VERSION);
        assert_eq!(migration_count, 2);
    }

    #[tokio::test]
    async fn disk_database_enables_wal_foreign_keys_and_busy_timeout() {
        let path = temporary_database_path("configuration");
        let database = Database::open(&path)
            .await
            .expect("disk database should initialize");

        let journal_mode: String = query_scalar("PRAGMA journal_mode")
            .fetch_one(&database.pool)
            .await
            .expect("journal mode should be readable");
        let foreign_keys: i64 = query_scalar("PRAGMA foreign_keys")
            .fetch_one(&database.pool)
            .await
            .expect("foreign key pragma should be readable");
        let busy_timeout: i64 = query_scalar("PRAGMA busy_timeout")
            .fetch_one(&database.pool)
            .await
            .expect("busy timeout pragma should be readable");

        assert_eq!(journal_mode, "wal");
        assert_eq!(foreign_keys, 1);
        assert_eq!(busy_timeout, 5_000);

        drop(database);
        remove_temporary_database(&path);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn disk_database_serializes_write_transactions_across_clones() {
        let path = temporary_database_path("write-gate");
        let database = Database::open(&path)
            .await
            .expect("disk database should initialize");
        let first = database
            .begin_write()
            .await
            .expect("first write transaction should begin");
        let second_database = database.clone();
        let (acquired_sender, mut acquired_receiver) = oneshot::channel();
        let (release_sender, release_receiver) = oneshot::channel();
        let second = tokio::spawn(async move {
            let transaction = second_database
                .begin_write()
                .await
                .expect("second write transaction should eventually begin");
            let _ = acquired_sender.send(());
            let _ = release_receiver.await;
            transaction
                .rollback()
                .await
                .expect("second transaction should roll back");
        });

        assert!(
            tokio::time::timeout(Duration::from_millis(100), &mut acquired_receiver)
                .await
                .is_err(),
            "a second write transaction must wait for the shared write gate"
        );
        first
            .rollback()
            .await
            .expect("first transaction should roll back");
        tokio::time::timeout(Duration::from_secs(1), &mut acquired_receiver)
            .await
            .expect("second transaction should acquire after release")
            .expect("second transaction should report acquisition");
        let _ = release_sender.send(());
        second.await.expect("second transaction task should join");

        database.pool.close().await;
        drop(database);
        remove_temporary_database(&path);
    }

    #[tokio::test]
    async fn existing_database_is_backed_up_before_startup() {
        let path = temporary_database_path("backup");
        let database = Database::open(&path)
            .await
            .expect("first disk database startup should initialize");
        drop(database);

        let database = Database::open(&path)
            .await
            .expect("second disk database startup should initialize");

        assert!(path.with_extension("bak").is_file());
        drop(database);
        remove_temporary_database(&path);
    }

    #[tokio::test]
    async fn foreign_keys_and_unique_constraints_are_enforced() {
        let database = Database::open_in_memory()
            .await
            .expect("temporary database should initialize");

        let foreign_key_error = query(
            "INSERT INTO file_records (
                id, project_id, relative_path, normalized_relative_path, size_bytes,
                source_status, included, result_status, scan_generation, created_at, updated_at
            ) VALUES ('file-missing', 'project-missing', 'src/main.rs', 'src/main.rs', 1,
                'normal', 1, 'none', 1, ?, ?)",
        )
        .bind(TIMESTAMP)
        .bind(TIMESTAMP)
        .execute(&database.pool)
        .await;
        assert!(foreign_key_error.is_err());

        insert_project(&database, "project-1", "/workspace/project").await;
        let duplicate_error = query(
            "INSERT INTO projects (
                id, schema_version, name, source_directory, canonical_source_directory,
                path_status, default_prompt, filter_rules_json, execution_defaults_json,
                api_routing_json, current_context_version_id, context_enabled, created_at,
                updated_at, last_opened_at
            ) VALUES ('project-2', 2, 'duplicate', '/workspace/duplicate', '/workspace/project',
                'available', 'prompt', '{}', '{}', '{}', NULL, 1, ?, ?, ?)",
        )
        .bind(TIMESTAMP)
        .bind(TIMESTAMP)
        .bind(TIMESTAMP)
        .execute(&database.pool)
        .await;
        assert!(duplicate_error.is_err());
    }

    #[tokio::test]
    async fn newer_schema_is_rejected_before_migrations_run() {
        let database = Database::open_in_memory()
            .await
            .expect("temporary database should initialize");

        query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
             VALUES (?, 'future', 1, X'00', 0)",
        )
        .bind(i64::from(LATEST_SCHEMA_VERSION) + 1)
        .execute(&database.pool)
        .await
        .expect("future migration marker should insert");

        assert_eq!(
            apply_migrations(&database.pool).await,
            Err(PersistenceError::SchemaTooNew {
                detected: LATEST_SCHEMA_VERSION + 1,
                supported: LATEST_SCHEMA_VERSION,
            })
        );
    }

    #[tokio::test]
    async fn newer_schema_enters_recovery_with_a_diagnostic_health_state() {
        let path = temporary_database_path("future-schema");
        let database = Database::open(&path)
            .await
            .expect("disk database should initialize");
        query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
             VALUES (?, 'future', 1, X'00', 0)",
        )
        .bind(i64::from(LATEST_SCHEMA_VERSION) + 1)
        .execute(&database.pool)
        .await
        .expect("future migration marker should insert");
        drop(database);

        let startup = Database::open_for_startup(&path).await;
        assert!(matches!(
            startup.health(),
            DatabaseHealth::MigrationFailed { schema_version }
                if schema_version == LATEST_SCHEMA_VERSION + 1
        ));
        let recovery = match startup {
            DatabaseStartup::Recovery(recovery) => recovery,
            DatabaseStartup::Ready(_) => panic!("future schema must not open read-write"),
        };
        let read_only = recovery
            .open_read_only()
            .await
            .expect("recovery database should open read-only");
        assert_eq!(read_only.schema_version(), LATEST_SCHEMA_VERSION + 1);
        drop(read_only);

        remove_temporary_database(&path);
    }

    #[tokio::test]
    async fn run_snapshot_is_immutable_and_run_status_uses_the_domain_state_machine() {
        let database = Database::open_in_memory()
            .await
            .expect("temporary database should initialize");
        insert_project(&database, "project-1", "/workspace/project").await;
        insert_run(&database, "run-1", "project-1", "draft").await;

        let snapshot_update =
            query("UPDATE runs SET snapshot_json = '{\"changed\":true}' WHERE id = 'run-1'")
                .execute(&database.pool)
                .await;
        assert!(snapshot_update.is_err());

        let status = database
            .transition_run_status(&RunId::new("run-1"), RunTransition::Start)
            .await
            .expect("documented transition should persist");
        assert_eq!(status, RunStatus::Running);
        assert!(matches!(
            database
                .transition_run_status(&RunId::new("run-1"), RunTransition::Start)
                .await,
            Err(PersistenceError::StateTransition {
                code: "run_invalid_transition"
            })
        ));
    }

    #[tokio::test]
    async fn attempts_preserve_identity_and_history_while_status_can_progress() {
        let database = Database::open_in_memory()
            .await
            .expect("temporary database should initialize");
        insert_project(&database, "project-1", "/workspace/project").await;
        insert_file(&database, "file-1", "project-1").await;
        insert_run(&database, "run-1", "project-1", "draft").await;
        insert_task(&database, "task-1", "run-1", "file-1").await;
        insert_attempt(&database, "attempt-1", "task-1", 1).await;

        query("UPDATE attempts SET status = 'dispatched' WHERE id = 'attempt-1'")
            .execute(&database.pool)
            .await
            .expect("current attempt status should progress");
        assert!(
            query("UPDATE attempts SET sequence = 2 WHERE id = 'attempt-1'")
                .execute(&database.pool)
                .await
                .is_err()
        );
        assert!(query("DELETE FROM attempts WHERE id = 'attempt-1'")
            .execute(&database.pool)
            .await
            .is_err());
        assert!(query(
            "INSERT INTO attempts (
                id, task_id, sequence, api_profile_id, api_profile_name_snapshot, actual_model,
                status, created_at, request_started_at
            ) VALUES ('attempt-2', 'task-1', 1, 'profile-1', 'Primary', 'gpt-5', 'created', ?, ?)",
        )
        .bind(TIMESTAMP)
        .bind(TIMESTAMP)
        .execute(&database.pool)
        .await
        .is_err());
    }

    async fn insert_project(database: &Database, id: &str, canonical_path: &str) {
        query(
            "INSERT INTO projects (
                id, schema_version, name, source_directory, canonical_source_directory,
                path_status, default_prompt, filter_rules_json, execution_defaults_json,
                api_routing_json, current_context_version_id, context_enabled, created_at,
                updated_at, last_opened_at
            ) VALUES (?, 2, 'example', '/workspace/project', ?, 'available', 'prompt', '{}', '{}',
                '{}', NULL, 1, ?, ?, ?)",
        )
        .bind(id)
        .bind(canonical_path)
        .bind(TIMESTAMP)
        .bind(TIMESTAMP)
        .bind(TIMESTAMP)
        .execute(&database.pool)
        .await
        .expect("project fixture should insert");
    }

    async fn insert_file(database: &Database, id: &str, project_id: &str) {
        query(
            "INSERT INTO file_records (
                id, project_id, relative_path, normalized_relative_path, size_bytes,
                source_status, included, result_status, scan_generation, created_at, updated_at
            ) VALUES (?, ?, 'src/main.rs', 'src/main.rs', 1, 'normal', 1, 'none', 1, ?, ?)",
        )
        .bind(id)
        .bind(project_id)
        .bind(TIMESTAMP)
        .bind(TIMESTAMP)
        .execute(&database.pool)
        .await
        .expect("file fixture should insert");
    }

    async fn insert_run(database: &Database, id: &str, project_id: &str, status: &str) {
        query(
            "INSERT INTO runs (
                id, project_id, status, context_version_id, output_directory, snapshot_json,
                stats_json, created_at, started_at, completed_at, interruption_reason
            ) VALUES (?, ?, ?, NULL, '/workspace/output', '{\"apiRouting\":{}}', '{}', ?, NULL, NULL, NULL)",
        )
        .bind(id)
        .bind(project_id)
        .bind(status)
        .bind(TIMESTAMP)
        .execute(&database.pool)
        .await
        .expect("run fixture should insert");
    }

    async fn insert_task(database: &Database, id: &str, run_id: &str, file_id: &str) {
        query(
            "INSERT INTO tasks (
                id, run_id, file_id, relative_path, file_snapshot_json, prompt_snapshot,
                prompt_hash, prompt_source, model_snapshot, model_source, context_version_id,
                status, current_result_path, latest_attempt_id, parent_task_id, result_version,
                created_at, started_at, completed_at
            ) VALUES (?, ?, ?, 'src/main.rs', '{}', 'prompt', 'sha256:prompt', 'project', 'gpt-5',
                'project', NULL, 'pending', NULL, NULL, NULL, 1, ?, NULL, NULL)",
        )
        .bind(id)
        .bind(run_id)
        .bind(file_id)
        .bind(TIMESTAMP)
        .execute(&database.pool)
        .await
        .expect("task fixture should insert");
    }

    async fn insert_attempt(database: &Database, id: &str, task_id: &str, sequence: i64) {
        query(
            "INSERT INTO attempts (
                id, task_id, sequence, api_profile_id, api_profile_name_snapshot, actual_model,
                status, created_at, request_started_at
            ) VALUES (?, ?, ?, 'profile-1', 'Primary', 'gpt-5', 'created', ?, ?)",
        )
        .bind(id)
        .bind(task_id)
        .bind(sequence)
        .bind(TIMESTAMP)
        .bind(TIMESTAMP)
        .execute(&database.pool)
        .await
        .expect("attempt fixture should insert");
    }

    fn temporary_database_path(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "batch-code-analyzer-{label}-{}-{timestamp}.db",
            std::process::id()
        ))
    }

    fn remove_temporary_database(path: &Path) {
        for candidate in [
            path.to_path_buf(),
            path.with_extension("bak"),
            path.with_extension("db-shm"),
            path.with_extension("db-wal"),
            path.with_extension("bak-shm"),
            path.with_extension("bak-wal"),
        ] {
            if candidate.exists() {
                fs::remove_file(candidate).expect("temporary database artifact should remove");
            }
        }
    }
}
