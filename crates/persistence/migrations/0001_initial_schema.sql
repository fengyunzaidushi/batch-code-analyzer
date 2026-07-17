CREATE TABLE projects (
  id TEXT PRIMARY KEY,
  schema_version INTEGER NOT NULL,
  name TEXT NOT NULL,
  source_directory TEXT NOT NULL,
  canonical_source_directory TEXT NOT NULL UNIQUE,
  path_status TEXT NOT NULL,
  default_prompt TEXT NOT NULL,
  default_model TEXT,
  context_model TEXT,
  output_root TEXT,
  filter_rules_json TEXT NOT NULL,
  execution_defaults_json TEXT NOT NULL,
  api_routing_json TEXT NOT NULL,
  current_context_version_id TEXT,
  context_enabled INTEGER NOT NULL DEFAULT 1,
  context_status TEXT NOT NULL DEFAULT 'ready',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_opened_at TEXT NOT NULL
);

CREATE TABLE api_profiles (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  protocol TEXT NOT NULL,
  base_url TEXT NOT NULL,
  key_reference_id TEXT,
  sensitive_header_reference_id TEXT,
  default_model TEXT,
  model_cache_json TEXT,
  model_cache_updated_at TEXT,
  last_connection_status TEXT,
  last_error_code TEXT,
  last_tested_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE file_records (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  relative_path TEXT NOT NULL,
  normalized_relative_path TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  modified_at TEXT,
  content_hash TEXT,
  encoding TEXT,
  language TEXT,
  source_status TEXT NOT NULL,
  included INTEGER NOT NULL,
  exclusion_reason TEXT,
  sensitive_findings_json TEXT NOT NULL DEFAULT '[]',
  result_status TEXT NOT NULL,
  latest_successful_run_id TEXT,
  latest_successful_task_id TEXT,
  scan_generation INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(project_id, normalized_relative_path)
);

CREATE TABLE context_versions (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  status TEXT NOT NULL,
  model TEXT,
  source_files_json TEXT NOT NULL,
  summary TEXT NOT NULL,
  summary_hash TEXT NOT NULL,
  manually_edited INTEGER NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE runs (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  status TEXT NOT NULL,
  context_version_id TEXT REFERENCES context_versions(id),
  output_directory TEXT NOT NULL,
  snapshot_json TEXT NOT NULL,
  stats_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  interruption_reason TEXT
);

CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
  file_id TEXT NOT NULL REFERENCES file_records(id),
  relative_path TEXT NOT NULL,
  file_snapshot_json TEXT NOT NULL,
  prompt_snapshot TEXT NOT NULL,
  prompt_hash TEXT NOT NULL,
  prompt_source TEXT NOT NULL,
  model_snapshot TEXT NOT NULL,
  model_source TEXT NOT NULL,
  context_version_id TEXT,
  status TEXT NOT NULL,
  current_result_path TEXT,
  latest_attempt_id TEXT,
  parent_task_id TEXT REFERENCES tasks(id),
  result_version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT
);

CREATE TABLE attempts (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  sequence INTEGER NOT NULL,
  api_profile_id TEXT NOT NULL,
  api_profile_name_snapshot TEXT NOT NULL,
  actual_model TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  request_started_at TEXT,
  request_dispatched_at TEXT,
  finished_at TEXT,
  duration_ms INTEGER,
  http_status INTEGER,
  input_tokens INTEGER,
  output_tokens INTEGER,
  total_tokens INTEGER,
  retry_reason TEXT,
  error_code TEXT,
  sanitized_error_message TEXT,
  error_retryable INTEGER,
  error_sanitized INTEGER,
  response_id TEXT,
  UNIQUE(task_id, sequence)
);

CREATE TABLE prompt_library (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  content TEXT NOT NULL,
  is_builtin INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_files_project_status
ON file_records(project_id, source_status, result_status);

CREATE INDEX idx_runs_project_created
ON runs(project_id, created_at DESC);

CREATE INDEX idx_tasks_run_status
ON tasks(run_id, status);

CREATE INDEX idx_tasks_file
ON tasks(file_id, created_at DESC);

CREATE INDEX idx_attempts_task_sequence
ON attempts(task_id, sequence);

CREATE TRIGGER prevent_run_snapshot_update
BEFORE UPDATE OF snapshot_json ON runs
FOR EACH ROW
BEGIN
  SELECT RAISE(ABORT, 'run_snapshot_immutable');
END;

CREATE TRIGGER prevent_attempt_identity_update
BEFORE UPDATE OF id, task_id, sequence ON attempts
FOR EACH ROW
BEGIN
  SELECT RAISE(ABORT, 'attempt_identity_immutable');
END;

CREATE TRIGGER prevent_attempt_delete
BEFORE DELETE ON attempts
FOR EACH ROW
BEGIN
  SELECT RAISE(ABORT, 'attempt_history_append_only');
END;
