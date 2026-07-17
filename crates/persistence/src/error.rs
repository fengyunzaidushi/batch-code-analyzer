use core::fmt;

/// Stable persistence errors without database-driver diagnostics or secrets.
#[derive(Debug, Eq, PartialEq)]
pub enum PersistenceError {
    DatabaseUnavailable,
    MigrationFailed,
    SchemaTooNew { detected: u32, supported: u32 },
    TransactionFailed,
    RecordNotFound { kind: &'static str, id: String },
    InvalidStoredState,
    StateTransition { code: &'static str },
}

impl PersistenceError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::DatabaseUnavailable => "persistence_database_unavailable",
            Self::MigrationFailed => "persistence_migration_failed",
            Self::SchemaTooNew { .. } => "persistence_schema_too_new",
            Self::TransactionFailed => "persistence_transaction_failed",
            Self::RecordNotFound { .. } => "persistence_record_not_found",
            Self::InvalidStoredState => "internal_contract_violation",
            Self::StateTransition { code } => code,
        }
    }
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PersistenceError {}
