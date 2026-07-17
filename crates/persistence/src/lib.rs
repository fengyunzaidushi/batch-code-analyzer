//! `SQLite` initialization, migrations, and transactional persistence primitives.

#![forbid(unsafe_code)]

mod database;
mod error;
pub mod repositories;
mod rows;

pub use database::{
    Database, DatabaseHealth, DatabaseStartup, ReadOnlyDatabase, RecoveryDatabase,
    WriteTransaction, LATEST_SCHEMA_VERSION,
};
pub use error::PersistenceError;
pub use repositories::Repository;
pub use rows::{
    AttemptRow, AttemptRowMetadata, ContextVersionRow, FileRecordRow, FileRecordRowMetadata,
    ProjectRow, ProjectRowMetadata, RunRow, RunRowMetadata, TaskRow,
};
