use batch_code_analyzer_ipc_contracts::HealthCheckResponse;

#[tauri::command]
pub(crate) fn health_check() -> HealthCheckResponse {
    HealthCheckResponse::ready(env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::health_check;

    #[test]
    fn health_check_reports_typed_bootstrap_state() {
        let response = health_check();

        assert_eq!(
            response.status,
            batch_code_analyzer_ipc_contracts::HealthStatus::Ready
        );
        assert_eq!(response.app_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            response.database_status,
            batch_code_analyzer_ipc_contracts::DatabaseStatus::NotInitialized
        );
        assert_eq!(response.database_schema_version, 0);
    }
}
