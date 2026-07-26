use std::{fs, io, path::Path};

const RESET_MARKER: &str = ".reset-user-data";

pub(crate) fn schedule_database_reset(database_path: &Path) -> io::Result<()> {
    let parent = database_path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "database path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!("{RESET_MARKER}.tmp-{}", std::process::id()));
    fs::write(&temporary, b"scheduled")?;
    fs::rename(temporary, parent.join(RESET_MARKER))
}

/// Applies a user-confirmed reset before `SQLite` is opened on the next start.
/// Missing files are treated as already cleared; the marker is retained until
/// every removal succeeds so a failed cleanup is retried on the next start.
pub(crate) fn apply_pending_database_reset(database_path: &Path) -> io::Result<bool> {
    let parent = database_path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "database path has no parent")
    })?;
    let marker = parent.join(RESET_MARKER);
    if !marker.is_file() {
        return Ok(false);
    }
    for path in [
        database_path.to_path_buf(),
        database_path.with_extension("bak"),
        database_path.with_extension("restore.tmp"),
        path_with_suffix(database_path, "-wal")?,
        path_with_suffix(database_path, "-shm")?,
        path_with_suffix(&database_path.with_extension("bak"), "-wal")?,
        path_with_suffix(&database_path.with_extension("bak"), "-shm")?,
    ] {
        if path.is_file() {
            fs::remove_file(path)?;
        }
    }
    fs::remove_file(marker)?;
    Ok(true)
}

/// Restores the last startup backup only when the primary database is missing.
/// An existing database is never replaced, including during an application
/// upgrade. `SQLite` will validate and migrate the restored file afterwards.
pub(crate) fn restore_missing_database_from_backup(database_path: &Path) -> io::Result<bool> {
    if database_path.exists() {
        return Ok(false);
    }
    let backup_path = database_path.with_extension("bak");
    if !backup_path.is_file() {
        return Ok(false);
    }
    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary_path = database_path.with_extension("restore.tmp");
    fs::copy(&backup_path, &temporary_path)?;
    for suffix in ["-wal", "-shm"] {
        let backup_sidecar = path_with_suffix(&backup_path, suffix)?;
        if backup_sidecar.is_file() {
            fs::copy(backup_sidecar, path_with_suffix(database_path, suffix)?)?;
        }
    }
    fs::rename(&temporary_path, database_path)?;
    Ok(true)
}

fn path_with_suffix(path: &Path, suffix: &str) -> io::Result<std::path::PathBuf> {
    let mut name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "database path has no name"))?
        .to_os_string();
    name.push(suffix);
    Ok(path.with_file_name(name))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        apply_pending_database_reset, restore_missing_database_from_backup,
        schedule_database_reset, RESET_MARKER,
    };

    fn temporary_directory(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("batch-code-analyzer-{name}-{nonce}"))
    }

    #[test]
    fn restores_backup_only_when_primary_database_is_missing() {
        let directory = temporary_directory("restore-backup");
        fs::create_dir_all(&directory).expect("temporary directory should exist");
        let database = directory.join("app.db");
        fs::write(database.with_extension("bak"), b"preserved data").expect("backup should exist");

        assert!(restore_missing_database_from_backup(&database).expect("restore should work"));
        assert_eq!(fs::read(&database).unwrap(), b"preserved data");

        fs::write(&database, b"current data").expect("primary should update");
        assert!(!restore_missing_database_from_backup(&database).expect("existing should remain"));
        assert_eq!(fs::read(&database).unwrap(), b"current data");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn reset_marker_removes_database_and_backup_before_restore() {
        let directory = temporary_directory("reset-marker");
        fs::create_dir_all(&directory).expect("temporary directory should exist");
        let database = directory.join("app.db");
        fs::write(&database, b"current").expect("database should exist");
        fs::write(database.with_extension("bak"), b"backup").expect("backup should exist");
        schedule_database_reset(&database).expect("reset should be scheduled");

        assert!(apply_pending_database_reset(&database).expect("reset should apply"));
        assert!(!database.exists());
        assert!(!database.with_extension("bak").exists());
        assert!(!directory.join(RESET_MARKER).exists());
        let _ = fs::remove_dir_all(directory);
    }
}
