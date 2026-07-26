use batch_code_analyzer_domain::{
    ApiProfile, ApiProfileConnectionStatus, ApiProfileId, ApiProtocol, ApiRouting, Attempt,
    AttemptError, AttemptId, AttemptStatus, ContextStatus, ExecutionDefaults, FileRecord,
    FileRecordId, FileResultStatus, FileSnapshot, FileSourceStatus, FilterRules, Project,
    ProjectContext, ProjectId, ProjectPathStatus, RetryPolicy, Rfc3339Timestamp, Run, RunId,
    RunSnapshot, RunStats, RunStatus, SensitiveFinding, Task, TaskId, TaskStatus, TaskValueSource,
};
use batch_code_analyzer_persistence::{
    AttemptRowMetadata, Database, FileRecordRowMetadata, PersistenceError, ProjectRowMetadata,
    RunRowMetadata,
};

const TIMESTAMP: &str = "2026-07-17T10:00:00+08:00";

#[tokio::test]
async fn project_repository_enforces_canonical_path_uniqueness() {
    let database = Database::open_in_memory()
        .await
        .expect("database should open");
    let repository = database.repository();
    let project_entity = project("project-1", "/workspace/project");

    repository
        .create_project(&project_entity, project_metadata("/workspace/project"))
        .await
        .expect("first project should insert");

    let duplicate = project("project-2", "/workspace/other");
    let error = repository
        .create_project(&duplicate, project_metadata("/workspace/project"))
        .await
        .expect_err("canonical path should be unique");
    assert_eq!(error.code(), "project_path_duplicate");

    assert_eq!(repository.list_projects().await.unwrap().len(), 1);
    assert_eq!(
        repository.get_project(&project_entity.id).await.unwrap(),
        Some(project_entity)
    );
    assert_eq!(
        repository
            .find_project_by_canonical_path("/workspace/project")
            .await
            .unwrap(),
        Some(project("project-1", "/workspace/project"))
    );
}

#[tokio::test]
async fn file_inclusion_updates_are_scoped_and_round_trip() {
    let database = Database::open_in_memory()
        .await
        .expect("database should open");
    let repository = database.repository();
    let project = project("project-1", "/workspace/project");
    repository
        .create_project(&project, project_metadata("/workspace/project"))
        .await
        .unwrap();
    let file = file("file-1", &project.id);
    repository
        .create_file_record(&file, file_metadata())
        .await
        .unwrap();

    let excluded = repository
        .set_file_included(&project.id, &file.id, false, &timestamp())
        .await
        .unwrap()
        .expect("file should be updated");
    assert!(!excluded.included);
    assert_eq!(excluded.exclusion_reason.as_deref(), Some("user_excluded"));

    let included = repository
        .set_file_included(&project.id, &file.id, true, &timestamp())
        .await
        .unwrap()
        .expect("file should be restored");
    assert!(included.included);
    assert_eq!(included.exclusion_reason, None);

    assert_eq!(
        repository
            .set_file_included(
                &ProjectId::new("other-project"),
                &file.id,
                false,
                &timestamp(),
            )
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn api_profile_repository_round_trips_and_blocks_referenced_delete() {
    let database = Database::open_in_memory()
        .await
        .expect("database should open");
    let repository = database.repository();
    let mut project = project("project-1", "/workspace/project");
    project.api_routing.primary_profile_id = Some(ApiProfileId::new("profile-1"));
    repository
        .create_project(&project, project_metadata("/workspace/project"))
        .await
        .unwrap();
    let profile = api_profile("profile-1");
    repository.create_api_profile(&profile).await.unwrap();
    assert_eq!(
        repository.list_api_profiles().await.unwrap(),
        vec![profile.clone()]
    );

    let mut updated = profile.clone();
    updated.default_model = Some("gpt-5-mini".into());
    updated.updated_at = timestamp();
    repository.update_api_profile(&updated).await.unwrap();
    assert_eq!(
        repository
            .get_api_profile(&updated.id)
            .await
            .unwrap()
            .unwrap()
            .default_model,
        Some("gpt-5-mini".into())
    );
    assert_eq!(
        repository
            .delete_api_profile(&updated.id)
            .await
            .expect_err("referenced profile must not be deleted")
            .code(),
        "api_profile_in_use"
    );
}

#[tokio::test]
async fn run_and_tasks_are_created_as_one_transaction() {
    let database = Database::open_in_memory()
        .await
        .expect("database should open");
    let repository = database.repository();
    let project = project("project-1", "/workspace/project");
    repository
        .create_project(&project, project_metadata("/workspace/project"))
        .await
        .unwrap();

    let run = run("run-1", &project.id, RunStatus::Draft);
    let missing_file_task = task("task-1", &run.id, "missing-file", TaskStatus::Pending);
    let error = repository
        .create_run_with_tasks(
            &run,
            RunRowMetadata {
                interruption_reason: None,
            },
            &[missing_file_task],
        )
        .await
        .expect_err("foreign key failure should roll back the run");
    assert_eq!(error.code(), "persistence_transaction_failed");
    assert_eq!(repository.get_run(&run.id).await.unwrap(), None);
}

#[tokio::test]
async fn queued_tasks_are_claimed_once_and_stats_are_recomputed_from_tasks() {
    let database = Database::open_in_memory()
        .await
        .expect("database should open");
    let repository = database.repository();
    let project = project("project-1", "/workspace/project");
    repository
        .create_project(&project, project_metadata("/workspace/project"))
        .await
        .unwrap();
    let file = file("file-1", &project.id);
    repository
        .create_file_record(&file, file_metadata())
        .await
        .unwrap();

    let active_run = run("run-1", &project.id, RunStatus::Running);
    let first = task(
        "task-1",
        &active_run.id,
        file.id.as_str(),
        TaskStatus::Queued,
    );
    let second = task(
        "task-2",
        &active_run.id,
        file.id.as_str(),
        TaskStatus::Pending,
    );
    let first_id = first.id.clone();
    let first_expected = first.clone();
    repository
        .create_run_with_tasks(
            &active_run,
            RunRowMetadata {
                interruption_reason: None,
            },
            &[first, second],
        )
        .await
        .unwrap();
    assert_eq!(
        repository.get_task(&first_id).await.unwrap().unwrap(),
        first_expected
    );
    let second_run = run("run-2", &project.id, RunStatus::Draft);
    let second_task = task(
        "task-3",
        &second_run.id,
        file.id.as_str(),
        TaskStatus::Pending,
    );
    assert_eq!(
        repository
            .create_run_with_tasks(
                &second_run,
                RunRowMetadata {
                    interruption_reason: None,
                },
                &[second_task],
            )
            .await
            .expect_err("only one active run is allowed")
            .code(),
        "run_active_exists"
    );

    let first_now = timestamp();
    let second_now = timestamp();
    let (first_claim, second_claim) = tokio::join!(
        repository.claim_next_task(&active_run.id, &first_now),
        repository.claim_next_task(&active_run.id, &second_now),
    );
    let claims = [first_claim.unwrap(), second_claim.unwrap()];
    assert_eq!(
        claims.iter().filter(|claim| claim.is_some()).count(),
        1,
        "one queued task must be claimed by at most one worker"
    );
    assert!(claims
        .iter()
        .filter_map(Option::as_ref)
        .all(|claim| claim.status == TaskStatus::Running));

    let stats = repository
        .recompute_run_stats(&active_run.id)
        .await
        .unwrap();
    assert_eq!(stats.total, 2);
    assert_eq!(stats.running, 1);
    assert_eq!(stats.pending, 1);
    assert_eq!(
        repository
            .get_run(&active_run.id)
            .await
            .unwrap()
            .unwrap()
            .stats,
        stats
    );
}

#[tokio::test]
async fn attempts_are_append_only_and_sequence_is_unique() {
    let database = Database::open_in_memory()
        .await
        .expect("database should open");
    let repository = database.repository();
    let project = project("project-1", "/workspace/project");
    repository
        .create_project(&project, project_metadata("/workspace/project"))
        .await
        .unwrap();
    let file = file("file-1", &project.id);
    repository
        .create_file_record(&file, file_metadata())
        .await
        .unwrap();
    let run = run("run-1", &project.id, RunStatus::Draft);
    let task = task("task-1", &run.id, file.id.as_str(), TaskStatus::Pending);
    repository
        .create_run_with_tasks(
            &run,
            RunRowMetadata {
                interruption_reason: None,
            },
            std::slice::from_ref(&task),
        )
        .await
        .unwrap();

    let attempt_entity = attempt("attempt-1", &task.id, 1, AttemptStatus::Created);
    repository
        .append_attempt(&attempt_entity, AttemptRowMetadata { response_id: None })
        .await
        .unwrap();
    let duplicate = attempt("attempt-2", &task.id, 1, AttemptStatus::Created);
    assert!(repository
        .append_attempt(&duplicate, AttemptRowMetadata { response_id: None })
        .await
        .is_err());
    assert_eq!(
        repository.list_attempts(&task.id).await.unwrap(),
        vec![attempt_entity]
    );
}

#[tokio::test]
async fn recovery_queries_find_only_unfinished_objects() {
    let database = Database::open_in_memory()
        .await
        .expect("database should open");
    let repository = database.repository();
    let project = project("project-1", "/workspace/project");
    repository
        .create_project(&project, project_metadata("/workspace/project"))
        .await
        .unwrap();
    let file = file("file-1", &project.id);
    repository
        .create_file_record(&file, file_metadata())
        .await
        .unwrap();

    let run = run("run-1", &project.id, RunStatus::Running);
    let task = task("task-1", &run.id, file.id.as_str(), TaskStatus::Running);
    repository
        .create_run_with_tasks(
            &run,
            RunRowMetadata {
                interruption_reason: None,
            },
            std::slice::from_ref(&task),
        )
        .await
        .unwrap();
    let attempt = attempt("attempt-1", &task.id, 1, AttemptStatus::Dispatched);
    repository
        .append_attempt(&attempt, AttemptRowMetadata { response_id: None })
        .await
        .unwrap();

    assert_eq!(repository.unfinished_runs().await.unwrap().len(), 1);
    assert_eq!(
        repository.unfinished_tasks(&run.id).await.unwrap(),
        vec![task.clone()]
    );
    assert_eq!(
        repository.unfinished_attempts(&task.id).await.unwrap(),
        vec![attempt]
    );
}

#[tokio::test]
async fn interrupting_run_settles_claimed_tasks_and_preserves_queue() {
    let database = Database::open_in_memory()
        .await
        .expect("database should open");
    let repository = database.repository();
    let project = project("project-interrupt", "/workspace/interrupt");
    repository
        .create_project(&project, project_metadata("/workspace/interrupt"))
        .await
        .unwrap();
    let file = file("file-interrupt", &project.id);
    repository
        .create_file_record(&file, file_metadata())
        .await
        .unwrap();
    let run = run("run-interrupt", &project.id, RunStatus::Running);
    let queued = task("task-queued", &run.id, file.id.as_str(), TaskStatus::Queued);
    let running = task(
        "task-running",
        &run.id,
        file.id.as_str(),
        TaskStatus::Running,
    );
    repository
        .create_run_with_tasks(
            &run,
            RunRowMetadata {
                interruption_reason: None,
            },
            &[queued, running.clone()],
        )
        .await
        .unwrap();
    repository
        .append_attempt(
            &attempt("attempt-running", &running.id, 1, AttemptStatus::Dispatched),
            AttemptRowMetadata { response_id: None },
        )
        .await
        .unwrap();

    let interrupted = repository
        .interrupt_run(&run.id, &timestamp())
        .await
        .expect("run should interrupt");

    assert_eq!(interrupted.status, RunStatus::Interrupted);
    assert_eq!(interrupted.stats.queued, 1);
    assert_eq!(interrupted.stats.interrupted, 1);
    let tasks = repository.list_tasks(&run.id).await.unwrap();
    assert!(tasks.iter().any(|task| task.status == TaskStatus::Queued));
    assert!(tasks
        .iter()
        .any(|task| task.status == TaskStatus::Interrupted));
    assert!(!tasks.iter().any(|task| task.status == TaskStatus::Running));
    assert_eq!(
        repository.list_attempts(&running.id).await.unwrap()[0].status,
        AttemptStatus::InterruptedUnknown
    );
}

struct BatchRetryFixture {
    database: Database,
    project: Project,
    file: FileRecord,
    retry_run: Run,
    first: Task,
    second: Task,
    terminal: Task,
}

async fn batch_retry_fixture() -> BatchRetryFixture {
    let database = Database::open_in_memory()
        .await
        .expect("database should open");
    let repository = database.repository();
    let project = project("project-batch-retry", "/workspace/batch-retry");
    repository
        .create_project(&project, project_metadata("/workspace/batch-retry"))
        .await
        .unwrap();
    let file = file("file-batch-retry", &project.id);
    repository
        .create_file_record(&file, file_metadata())
        .await
        .unwrap();

    let retry_run = run(
        "run-batch-retry",
        &project.id,
        RunStatus::CompletedWithErrors,
    );
    let mut first = task(
        "task-batch-retry-1",
        &retry_run.id,
        file.id.as_str(),
        TaskStatus::Failed,
    );
    let mut second = task(
        "task-batch-retry-2",
        &retry_run.id,
        file.id.as_str(),
        TaskStatus::Failed,
    );
    let mut terminal = task(
        "task-batch-retry-terminal",
        &retry_run.id,
        file.id.as_str(),
        TaskStatus::Failed,
    );
    first.latest_attempt_id = Some(AttemptId::new("attempt-batch-retry-1"));
    second.latest_attempt_id = Some(AttemptId::new("attempt-batch-retry-2"));
    terminal.latest_attempt_id = Some(AttemptId::new("attempt-batch-retry-terminal"));
    repository
        .create_run_with_tasks(
            &retry_run,
            RunRowMetadata {
                interruption_reason: None,
            },
            &[first.clone(), second.clone(), terminal.clone()],
        )
        .await
        .unwrap();
    for (id, task_id, retryable) in [
        ("attempt-batch-retry-1", &first.id, true),
        ("attempt-batch-retry-2", &second.id, true),
        ("attempt-batch-retry-terminal", &terminal.id, false),
    ] {
        let mut failed_attempt = attempt(id, task_id, 1, AttemptStatus::FailedTerminal);
        failed_attempt.error = Some(AttemptError {
            code: "provider_http_error".into(),
            message: "request failed".into(),
            retryable,
            sanitized: true,
        });
        repository
            .append_attempt(&failed_attempt, AttemptRowMetadata { response_id: None })
            .await
            .unwrap();
    }

    BatchRetryFixture {
        database,
        project,
        file,
        retry_run,
        first,
        second,
        terminal,
    }
}

#[tokio::test]
async fn batch_retry_rolls_back_cross_run_input() {
    let fixture = batch_retry_fixture().await;
    let repository = fixture.database.repository();

    let other_run = run(
        "run-batch-retry-other",
        &fixture.project.id,
        RunStatus::CompletedWithErrors,
    );
    let cross_run = task(
        "task-batch-retry-other",
        &other_run.id,
        fixture.file.id.as_str(),
        TaskStatus::Failed,
    );
    repository
        .create_run_with_tasks(
            &other_run,
            RunRowMetadata {
                interruption_reason: None,
            },
            std::slice::from_ref(&cross_run),
        )
        .await
        .unwrap();

    assert!(matches!(
        repository
            .retry_failed_tasks(
                &fixture.retry_run.id,
                &[fixture.first.id.clone(), cross_run.id]
            )
            .await
            .expect_err("cross-Run input must roll back the whole batch"),
        PersistenceError::RecordNotFound { kind: "task", .. }
    ));
    assert_eq!(
        repository
            .get_task(&fixture.first.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Failed
    );
    assert_eq!(
        repository
            .get_run(&fixture.retry_run.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        RunStatus::CompletedWithErrors
    );
}

#[tokio::test]
async fn batch_retry_requeues_eligible_tasks_and_skips_terminal_failures() {
    let fixture = batch_retry_fixture().await;
    let repository = fixture.database.repository();

    let (reopened, requeued, skipped) = repository
        .retry_failed_tasks(
            &fixture.retry_run.id,
            &[
                fixture.first.id.clone(),
                fixture.second.id.clone(),
                fixture.terminal.id.clone(),
            ],
        )
        .await
        .expect("eligible failed Tasks should requeue together");
    assert_eq!(reopened.status, RunStatus::Running);
    assert_eq!(reopened.stats.queued, 2);
    assert_eq!(reopened.stats.failed, 1);
    assert_eq!(
        requeued.iter().map(|task| &task.id).collect::<Vec<_>>(),
        vec![&fixture.first.id, &fixture.second.id]
    );
    assert_eq!(skipped, vec![fixture.terminal.id.clone()]);
    assert_eq!(
        repository
            .list_attempts(&fixture.first.id)
            .await
            .unwrap()
            .len(),
        1,
        "requeue must not create an Attempt before dispatch"
    );
}

#[tokio::test]
async fn cancelling_run_settles_queued_and_running_tasks_atomically() {
    let database = Database::open_in_memory()
        .await
        .expect("database should open");
    let repository = database.repository();
    let project = project("project-cancel", "/workspace/cancel");
    repository
        .create_project(&project, project_metadata("/workspace/cancel"))
        .await
        .unwrap();
    let file = file("file-cancel", &project.id);
    repository
        .create_file_record(&file, file_metadata())
        .await
        .unwrap();
    let run = run("run-cancel", &project.id, RunStatus::Running);
    let queued = task("task-queued", &run.id, file.id.as_str(), TaskStatus::Queued);
    let running = task(
        "task-running",
        &run.id,
        file.id.as_str(),
        TaskStatus::Running,
    );
    repository
        .create_run_with_tasks(
            &run,
            RunRowMetadata {
                interruption_reason: None,
            },
            &[queued, running.clone()],
        )
        .await
        .unwrap();
    repository
        .append_attempt(
            &attempt("attempt-running", &running.id, 1, AttemptStatus::Dispatched),
            AttemptRowMetadata { response_id: None },
        )
        .await
        .unwrap();

    let cancelled = repository
        .cancel_run(&run.id, &timestamp())
        .await
        .expect("run should cancel");
    assert_eq!(cancelled.status, RunStatus::Cancelled);
    assert_eq!(cancelled.stats.cancelled, 1);
    assert_eq!(cancelled.stats.interrupted, 1);
    assert!(repository.unfinished_runs().await.unwrap().is_empty());
    let tasks = repository.list_tasks(&run.id).await.unwrap();
    assert!(tasks
        .iter()
        .any(|task| task.status == TaskStatus::Cancelled));
    assert!(tasks
        .iter()
        .any(|task| task.status == TaskStatus::Interrupted));
    assert_eq!(
        repository.list_attempts(&running.id).await.unwrap()[0].status,
        AttemptStatus::InterruptedUnknown
    );
    assert_eq!(
        repository
            .cancel_run(&run.id, &timestamp())
            .await
            .expect_err("terminal run cannot be cancelled")
            .code(),
        "run_not_active"
    );
}

#[tokio::test]
async fn cancelling_run_settles_three_hundred_queued_tasks_without_attempts() {
    const TASK_COUNT: u32 = 300;
    const TASK_COUNT_USIZE: usize = 300;

    let database = Database::open_in_memory()
        .await
        .expect("database should open");
    let repository = database.repository();
    let project = project("project-bulk-cancel", "/workspace/bulk-cancel");
    repository
        .create_project(&project, project_metadata("/workspace/bulk-cancel"))
        .await
        .unwrap();
    let file = file("file-bulk-cancel", &project.id);
    repository
        .create_file_record(&file, file_metadata())
        .await
        .unwrap();
    let run = run("run-bulk-cancel", &project.id, RunStatus::Running);
    let queued = (0..TASK_COUNT)
        .map(|index| {
            task(
                &format!("task-bulk-cancel-{index}"),
                &run.id,
                file.id.as_str(),
                TaskStatus::Queued,
            )
        })
        .collect::<Vec<_>>();
    repository
        .create_run_with_tasks(
            &run,
            RunRowMetadata {
                interruption_reason: None,
            },
            &queued,
        )
        .await
        .unwrap();

    let cancelled = repository
        .cancel_run(&run.id, &timestamp())
        .await
        .expect("bulk Run should cancel");
    assert_eq!(cancelled.status, RunStatus::Cancelled);
    assert_eq!(cancelled.stats.cancelled, TASK_COUNT);
    assert_eq!(cancelled.stats.queued, 0);
    assert_eq!(cancelled.stats.running, 0);
    let tasks = repository.list_tasks(&run.id).await.unwrap();
    assert_eq!(tasks.len(), TASK_COUNT_USIZE);
    assert!(tasks
        .iter()
        .all(|task| task.status == TaskStatus::Cancelled));
    for task in &tasks {
        assert!(repository.list_attempts(&task.id).await.unwrap().is_empty());
    }
}

fn timestamp() -> Rfc3339Timestamp {
    Rfc3339Timestamp::new(TIMESTAMP)
}

fn project(id: &str, canonical_path: &str) -> Project {
    Project {
        schema_version: 2,
        id: ProjectId::new(id),
        name: "example".into(),
        source_directory: canonical_path.into(),
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
        output_root: Some("/workspace/results".into()),
        last_opened_at: timestamp(),
    }
}

fn project_metadata(canonical_path: &str) -> ProjectRowMetadata {
    ProjectRowMetadata {
        canonical_source_directory: canonical_path.into(),
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

fn file(id: &str, project_id: &ProjectId) -> FileRecord {
    FileRecord {
        id: FileRecordId::new(id),
        project_id: project_id.clone(),
        relative_path: "src/main.rs".into(),
        size_bytes: 10,
        modified_at: Some(timestamp()),
        content_hash: Some("blake3:file".into()),
        encoding: Some("utf-8".into()),
        language: Some("rust".into()),
        source_status: FileSourceStatus::Normal,
        included: true,
        exclusion_reason: None,
        sensitive_findings: vec![SensitiveFinding {
            kind: "none".into(),
            line: None,
            column: None,
        }],
        latest_successful_run_id: None,
        result_status: FileResultStatus::None,
    }
}

fn api_profile(id: &str) -> ApiProfile {
    ApiProfile {
        id: ApiProfileId::new(id),
        name: "Test Profile".into(),
        protocol: ApiProtocol::OpenAiResponses,
        base_url: "https://example.test/v1".into(),
        secret_ref: Some("session-secret-1".into()),
        default_model: Some("gpt-5".into()),
        model_cache: Vec::new(),
        model_cache_updated_at: None,
        last_connection_status: ApiProfileConnectionStatus::Unknown,
        last_error_code: None,
        last_tested_at: None,
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

fn file_metadata() -> FileRecordRowMetadata {
    FileRecordRowMetadata {
        normalized_relative_path: "src/main.rs".into(),
        latest_successful_task_id: None,
        scan_generation: 1,
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

fn run(id: &str, project_id: &ProjectId, status: RunStatus) -> Run {
    Run {
        id: RunId::new(id),
        project_id: project_id.clone(),
        status,
        created_at: timestamp(),
        started_at: None,
        completed_at: None,
        context_version_id: None,
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
        stats: RunStats::default(),
    }
}

fn task(id: &str, run_id: &RunId, file_id: &str, status: TaskStatus) -> Task {
    Task {
        id: TaskId::new(id),
        run_id: run_id.clone(),
        file_id: FileRecordId::new(file_id),
        relative_path: "src/main.rs".into(),
        file_snapshot: FileSnapshot {
            content_hash: "blake3:file".into(),
            size_bytes: 10,
        },
        prompt_snapshot: "Explain this file".into(),
        prompt_hash: "sha256:prompt".into(),
        prompt_source: TaskValueSource::Project,
        model_snapshot: "gpt-5".into(),
        model_source: TaskValueSource::Project,
        context_version_id: None,
        status,
        current_result_path: None,
        latest_attempt_id: None,
        parent_task_id: None,
        result_version: 1,
        created_at: timestamp(),
        started_at: None,
        completed_at: None,
    }
}

fn attempt(id: &str, task_id: &TaskId, sequence: u32, status: AttemptStatus) -> Attempt {
    Attempt {
        id: AttemptId::new(id),
        task_id: task_id.clone(),
        sequence,
        api_profile_id: ApiProfileId::new("profile-1"),
        api_profile_name: "Primary".into(),
        actual_model: "gpt-5".into(),
        status,
        created_at: timestamp(),
        started_at: Some(timestamp()),
        dispatched_at: Some(timestamp()),
        finished_at: None,
        duration_ms: None,
        http_status: None,
        input_tokens: None,
        output_tokens: None,
        total_tokens: None,
        retry_reason: None,
        error: None,
    }
}

#[allow(dead_code)]
fn _assert_error_type(_: PersistenceError) {}
