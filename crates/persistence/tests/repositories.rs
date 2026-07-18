use batch_code_analyzer_domain::{
    ApiProfileId, ApiRouting, Attempt, AttemptId, AttemptStatus, ContextStatus, ExecutionDefaults,
    FileRecord, FileRecordId, FileResultStatus, FileSnapshot, FileSourceStatus, FilterRules,
    Project, ProjectContext, ProjectId, ProjectPathStatus, RetryPolicy, Rfc3339Timestamp, Run,
    RunId, RunSnapshot, RunStats, RunStatus, SensitiveFinding, Task, TaskId, TaskStatus,
    TaskValueSource,
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

    let run = run("run-1", &project.id, RunStatus::Running);
    let first = task("task-1", &run.id, file.id.as_str(), TaskStatus::Queued);
    let second = task("task-2", &run.id, file.id.as_str(), TaskStatus::Pending);
    repository
        .create_run_with_tasks(
            &run,
            RunRowMetadata {
                interruption_reason: None,
            },
            &[first, second],
        )
        .await
        .unwrap();

    let first_now = timestamp();
    let second_now = timestamp();
    let (first_claim, second_claim) = tokio::join!(
        repository.claim_next_task(&run.id, &first_now),
        repository.claim_next_task(&run.id, &second_now),
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

    let stats = repository.recompute_run_stats(&run.id).await.unwrap();
    assert_eq!(stats.total, 2);
    assert_eq!(stats.running, 1);
    assert_eq!(stats.pending, 1);
    assert_eq!(
        repository.get_run(&run.id).await.unwrap().unwrap().stats,
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
