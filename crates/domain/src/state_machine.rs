use core::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Draft,
    Running,
    Pausing,
    Paused,
    Cancelling,
    Cancelled,
    Completed,
    CompletedWithErrors,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RunTransition {
    Start,
    PauseRequested,
    ActiveTasksDrained,
    Resume,
    CancelRequested,
    AllTasksTerminal,
    AllTasksSucceeded,
    AllTasksTerminalWithErrors,
    ProcessInterrupted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
    SourceChanged,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TaskTransition {
    Enqueue,
    Cancel,
    Claim,
    Succeed,
    Fail,
    CancelConfirmed,
    ProcessInterrupted,
    SourceHashChanged,
    ManualRetry,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AttemptStatus {
    Created,
    Dispatched,
    Succeeded,
    FailedRetryable,
    FailedTerminal,
    Cancelled,
    InterruptedUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateTransitionError {
    code: &'static str,
}

impl StateTransitionError {
    const fn run_invalid_transition() -> Self {
        Self {
            code: "run_invalid_transition",
        }
    }

    const fn task_invalid_transition() -> Self {
        Self {
            code: "task_invalid_transition",
        }
    }

    #[must_use]
    pub const fn code(self) -> &'static str {
        self.code
    }
}

impl fmt::Display for StateTransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

impl std::error::Error for StateTransitionError {}

pub struct RunStateMachine;

impl RunStateMachine {
    /// Validates and applies a documented Run state transition.
    ///
    /// # Errors
    ///
    /// Returns `run_invalid_transition` when the event is not valid for the
    /// current Run status.
    pub const fn transition(
        from: RunStatus,
        event: RunTransition,
    ) -> Result<RunStatus, StateTransitionError> {
        match (from, event) {
            (RunStatus::Draft, RunTransition::Start)
            | (RunStatus::Paused, RunTransition::Resume) => Ok(RunStatus::Running),
            (RunStatus::Running, RunTransition::PauseRequested) => Ok(RunStatus::Pausing),
            (RunStatus::Pausing, RunTransition::ActiveTasksDrained) => Ok(RunStatus::Paused),
            (
                RunStatus::Running | RunStatus::Pausing | RunStatus::Paused,
                RunTransition::CancelRequested,
            ) => Ok(RunStatus::Cancelling),
            (RunStatus::Cancelling, RunTransition::AllTasksTerminal) => Ok(RunStatus::Cancelled),
            (RunStatus::Running, RunTransition::AllTasksSucceeded) => Ok(RunStatus::Completed),
            (RunStatus::Running, RunTransition::AllTasksTerminalWithErrors) => {
                Ok(RunStatus::CompletedWithErrors)
            }
            (
                RunStatus::Running | RunStatus::Pausing | RunStatus::Cancelling,
                RunTransition::ProcessInterrupted,
            ) => Ok(RunStatus::Interrupted),
            _ => Err(StateTransitionError::run_invalid_transition()),
        }
    }
}

pub struct TaskStateMachine;

impl TaskStateMachine {
    /// Validates and applies a documented Task state transition.
    ///
    /// # Errors
    ///
    /// Returns `task_invalid_transition` when the event is not valid for the
    /// current Task status.
    pub const fn transition(
        from: TaskStatus,
        event: TaskTransition,
    ) -> Result<TaskStatus, StateTransitionError> {
        match (from, event) {
            (TaskStatus::Pending, TaskTransition::Enqueue)
            | (
                TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Interrupted,
                TaskTransition::ManualRetry,
            ) => Ok(TaskStatus::Queued),
            (TaskStatus::Pending | TaskStatus::Queued, TaskTransition::Cancel) => {
                Ok(TaskStatus::Cancelled)
            }
            (TaskStatus::Queued, TaskTransition::Claim) => Ok(TaskStatus::Running),
            (TaskStatus::Running, TaskTransition::Succeed) => Ok(TaskStatus::Succeeded),
            (TaskStatus::Running, TaskTransition::Fail) => Ok(TaskStatus::Failed),
            (TaskStatus::Running, TaskTransition::CancelConfirmed) => Ok(TaskStatus::Cancelled),
            (TaskStatus::Running, TaskTransition::ProcessInterrupted) => {
                Ok(TaskStatus::Interrupted)
            }
            (TaskStatus::Pending | TaskStatus::Queued, TaskTransition::SourceHashChanged) => {
                Ok(TaskStatus::SourceChanged)
            }
            _ => Err(StateTransitionError::task_invalid_transition()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RunStateMachine, RunStatus, RunTransition, TaskStateMachine, TaskStatus, TaskTransition,
    };

    #[test]
    fn run_state_machine_accepts_every_documented_transition() {
        let transitions = [
            (RunStatus::Draft, RunTransition::Start, RunStatus::Running),
            (
                RunStatus::Running,
                RunTransition::PauseRequested,
                RunStatus::Pausing,
            ),
            (
                RunStatus::Pausing,
                RunTransition::ActiveTasksDrained,
                RunStatus::Paused,
            ),
            (RunStatus::Paused, RunTransition::Resume, RunStatus::Running),
            (
                RunStatus::Running,
                RunTransition::CancelRequested,
                RunStatus::Cancelling,
            ),
            (
                RunStatus::Pausing,
                RunTransition::CancelRequested,
                RunStatus::Cancelling,
            ),
            (
                RunStatus::Paused,
                RunTransition::CancelRequested,
                RunStatus::Cancelling,
            ),
            (
                RunStatus::Cancelling,
                RunTransition::AllTasksTerminal,
                RunStatus::Cancelled,
            ),
            (
                RunStatus::Running,
                RunTransition::AllTasksSucceeded,
                RunStatus::Completed,
            ),
            (
                RunStatus::Running,
                RunTransition::AllTasksTerminalWithErrors,
                RunStatus::CompletedWithErrors,
            ),
            (
                RunStatus::Running,
                RunTransition::ProcessInterrupted,
                RunStatus::Interrupted,
            ),
            (
                RunStatus::Pausing,
                RunTransition::ProcessInterrupted,
                RunStatus::Interrupted,
            ),
            (
                RunStatus::Cancelling,
                RunTransition::ProcessInterrupted,
                RunStatus::Interrupted,
            ),
        ];

        for (from, event, expected) in transitions {
            assert_eq!(RunStateMachine::transition(from, event), Ok(expected));
        }
    }

    #[test]
    fn run_state_machine_rejects_illegal_transition_with_stable_code() {
        let result = RunStateMachine::transition(RunStatus::Completed, RunTransition::Start);

        assert!(matches!(result, Err(error) if error.code() == "run_invalid_transition"));
    }

    #[test]
    fn task_state_machine_accepts_every_documented_transition() {
        let transitions = [
            (
                TaskStatus::Pending,
                TaskTransition::Enqueue,
                TaskStatus::Queued,
            ),
            (
                TaskStatus::Pending,
                TaskTransition::Cancel,
                TaskStatus::Cancelled,
            ),
            (
                TaskStatus::Queued,
                TaskTransition::Cancel,
                TaskStatus::Cancelled,
            ),
            (
                TaskStatus::Queued,
                TaskTransition::Claim,
                TaskStatus::Running,
            ),
            (
                TaskStatus::Running,
                TaskTransition::Succeed,
                TaskStatus::Succeeded,
            ),
            (
                TaskStatus::Running,
                TaskTransition::Fail,
                TaskStatus::Failed,
            ),
            (
                TaskStatus::Running,
                TaskTransition::CancelConfirmed,
                TaskStatus::Cancelled,
            ),
            (
                TaskStatus::Running,
                TaskTransition::ProcessInterrupted,
                TaskStatus::Interrupted,
            ),
            (
                TaskStatus::Pending,
                TaskTransition::SourceHashChanged,
                TaskStatus::SourceChanged,
            ),
            (
                TaskStatus::Queued,
                TaskTransition::SourceHashChanged,
                TaskStatus::SourceChanged,
            ),
            (
                TaskStatus::Failed,
                TaskTransition::ManualRetry,
                TaskStatus::Queued,
            ),
            (
                TaskStatus::Cancelled,
                TaskTransition::ManualRetry,
                TaskStatus::Queued,
            ),
            (
                TaskStatus::Interrupted,
                TaskTransition::ManualRetry,
                TaskStatus::Queued,
            ),
        ];

        for (from, event, expected) in transitions {
            assert_eq!(TaskStateMachine::transition(from, event), Ok(expected));
        }
    }

    #[test]
    fn task_state_machine_rejects_illegal_transition_with_stable_code() {
        let result =
            TaskStateMachine::transition(TaskStatus::Succeeded, TaskTransition::ManualRetry);

        assert!(matches!(result, Err(error) if error.code() == "task_invalid_transition"));
    }
}
