use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use batch_code_analyzer_app_core::domain::ProjectId;
use batch_code_analyzer_ipc_contracts::{ScanOperationStatus, ScanReportDto};
use batch_code_analyzer_repository_scanner::ScanCancellation;

#[derive(Clone, Default)]
pub(crate) struct ScanState {
    operations: Arc<Mutex<HashMap<String, ScanOperation>>>,
}

#[derive(Debug)]
struct ScanOperation {
    project_id: ProjectId,
    cancellation: ScanCancellation,
    report: ScanReportDto,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanStateError {
    AlreadyRunning,
    NotFound,
}

impl ScanState {
    pub(crate) fn begin(
        &self,
        operation_id: String,
        project_id: ProjectId,
        cancellation: ScanCancellation,
        report: ScanReportDto,
    ) -> Result<(), ScanStateError> {
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| ScanStateError::AlreadyRunning)?;
        if operations.values().any(|operation| {
            operation.project_id == project_id
                && operation.report.status == ScanOperationStatus::Running
        }) {
            return Err(ScanStateError::AlreadyRunning);
        }
        operations.insert(
            operation_id,
            ScanOperation {
                project_id,
                cancellation,
                report,
            },
        );
        Ok(())
    }

    pub(crate) fn cancel(&self, operation_id: &str) -> Result<bool, ScanStateError> {
        let operations = self
            .operations
            .lock()
            .map_err(|_| ScanStateError::NotFound)?;
        let operation = operations
            .get(operation_id)
            .ok_or(ScanStateError::NotFound)?;
        if operation.report.status != ScanOperationStatus::Running {
            return Ok(false);
        }
        operation.cancellation.cancel();
        Ok(true)
    }

    pub(crate) fn update(
        &self,
        operation_id: &str,
        report: ScanReportDto,
    ) -> Result<(), ScanStateError> {
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| ScanStateError::NotFound)?;
        let operation = operations
            .get_mut(operation_id)
            .ok_or(ScanStateError::NotFound)?;
        operation.report = report;
        Ok(())
    }

    pub(crate) fn report(&self, operation_id: &str) -> Result<ScanReportDto, ScanStateError> {
        let operations = self
            .operations
            .lock()
            .map_err(|_| ScanStateError::NotFound)?;
        operations
            .get(operation_id)
            .map(|operation| operation.report.clone())
            .ok_or(ScanStateError::NotFound)
    }
}
