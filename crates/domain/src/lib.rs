//! Framework-independent domain identifiers, statuses, and state machines.

#![forbid(unsafe_code)]

mod entities;
mod ids;
mod state_machine;

pub use entities::{
    ApiFallback, ApiRouting, Attempt, AttemptError, ContextStatus, ContextVersion,
    ContextVersionSourceFile, ExecutionDefaults, FileRecord, FileResultStatus, FileSnapshot,
    FileSourceStatus, FilterRules, ModelRoutingStrategy, Project, ProjectContext,
    ProjectPathStatus, RetryPolicy, Rfc3339Timestamp, Run, RunSnapshot, RunStats, SensitiveFinding,
    Task, TaskValueSource,
};
pub use ids::{ApiProfileId, AttemptId, ContextVersionId, FileRecordId, ProjectId, RunId, TaskId};
pub use state_machine::{
    AttemptStatus, RunStateMachine, RunStatus, RunTransition, StateTransitionError,
    TaskStateMachine, TaskStatus, TaskTransition,
};
