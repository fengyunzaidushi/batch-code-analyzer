const HEALTHY_RESPONSE: &str = "ok";

#[tauri::command]
pub(crate) const fn health_check() -> &'static str {
    HEALTHY_RESPONSE
}

#[cfg(test)]
mod tests {
    use super::health_check;

    #[test]
    fn health_check_reports_ok() {
        assert_eq!(health_check(), "ok");
    }
}
