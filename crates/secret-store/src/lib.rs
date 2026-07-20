//! Secret storage interfaces, an OS keyring backend, and a process-local test
//! implementation.
//!
//! The secret value intentionally has no `Debug` or `Serialize` implementation,
//! so it cannot accidentally cross a logging or DTO boundary.

#![forbid(unsafe_code)]

use std::{
    collections::HashMap,
    fmt,
    path::Path,
    sync::{atomic::AtomicU64, Arc},
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::sync::atomic::Ordering;
use tokio::sync::RwLock;

/// Opaque reference stored in ordinary configuration instead of a secret.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SecretRef {
    id: String,
}

impl SecretRef {
    /// Creates a reference from an opaque identifier returned by a backend.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.id
    }
}

impl fmt::Debug for SecretRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretRef")
            .field("id", &self.id)
            .finish()
    }
}

impl fmt::Display for SecretRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.id)
    }
}

/// A secret value that cannot be formatted or serialized by accident.
#[derive(Clone, Eq, PartialEq)]
pub struct SecretValue(String);

impl SecretValue {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SecretValue {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecretValue {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// Runtime capability of the selected secure storage backend.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretStoreAvailability {
    Available,
    SessionOnly,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretError {
    Unavailable,
    NotFound,
    InvalidReference,
    BackendFailure,
}

impl SecretError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "security_secret_store_unavailable",
            Self::NotFound | Self::InvalidReference => "security_secret_not_found",
            Self::BackendFailure => "security_secret_store_failure",
        }
    }
}

impl fmt::Display for SecretError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SecretError {}

/// Backend abstraction used by providers and application services.
#[async_trait]
pub trait SecretStore: Send + Sync {
    fn availability(&self) -> SecretStoreAvailability;

    async fn put(&self, secret: SecretValue) -> Result<SecretRef, SecretError>;

    async fn get(&self, reference: &SecretRef) -> Result<SecretValue, SecretError>;

    async fn delete(&self, reference: &SecretRef) -> Result<(), SecretError>;
}

/// In-memory store intended for tests and an explicitly session-only mode.
#[derive(Clone)]
pub struct MemorySecretStore {
    values: Arc<RwLock<HashMap<String, SecretValue>>>,
    next_id: Arc<AtomicU64>,
    availability: SecretStoreAvailability,
}

impl Default for MemorySecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemorySecretStore {
    #[must_use]
    pub fn new() -> Self {
        Self::with_availability(SecretStoreAvailability::Available)
    }

    #[must_use]
    pub fn with_availability(availability: SecretStoreAvailability) -> Self {
        Self {
            values: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            availability,
        }
    }
}

#[async_trait]
impl SecretStore for MemorySecretStore {
    fn availability(&self) -> SecretStoreAvailability {
        self.availability
    }

    async fn put(&self, secret: SecretValue) -> Result<SecretRef, SecretError> {
        if self.availability == SecretStoreAvailability::Unavailable {
            return Err(SecretError::Unavailable);
        }
        let id = format!(
            "session-secret-{}",
            self.next_id.fetch_add(1, Ordering::Relaxed)
        );
        self.values.write().await.insert(id.clone(), secret);
        Ok(SecretRef::new(id))
    }

    async fn get(&self, reference: &SecretRef) -> Result<SecretValue, SecretError> {
        if self.availability == SecretStoreAvailability::Unavailable {
            return Err(SecretError::Unavailable);
        }
        if reference.as_str().is_empty() {
            return Err(SecretError::InvalidReference);
        }
        self.values
            .read()
            .await
            .get(reference.as_str())
            .cloned()
            .ok_or(SecretError::NotFound)
    }

    async fn delete(&self, reference: &SecretRef) -> Result<(), SecretError> {
        if self.availability == SecretStoreAvailability::Unavailable {
            return Err(SecretError::Unavailable);
        }
        if self
            .values
            .write()
            .await
            .remove(reference.as_str())
            .is_some()
        {
            Ok(())
        } else {
            Err(SecretError::NotFound)
        }
    }
}

/// Persistent `SecretStore` backed by the operating system credential manager.
///
/// The keyring entry username is the opaque `SecretRef`; the API key itself is
/// never represented by a serializable application type or persisted in `SQLite`.
pub struct KeyringSecretStore {
    service: String,
}

static NEXT_KEYRING_SECRET_ID: AtomicU64 = AtomicU64::new(1);

impl KeyringSecretStore {
    pub const DEFAULT_SERVICE: &'static str = "com.batchcodeanalyzer.desktop";

    /// Initializes the platform credential backend without creating a secret.
    ///
    /// # Errors
    ///
    /// Returns `SecretError::Unavailable` when the operating system has no
    /// usable credential backend, or `SecretError::BackendFailure` when the
    /// backend cannot be initialized.
    pub fn new() -> Result<Self, SecretError> {
        let service = Self::DEFAULT_SERVICE.to_owned();
        keyring::Entry::new(&service, "__backend_probe__")
            .map_err(|error| map_keyring_error(&error))?;
        Ok(Self { service })
    }

    fn entry(&self, reference: &SecretRef) -> Result<keyring::Entry, SecretError> {
        if reference.as_str().is_empty() {
            return Err(SecretError::InvalidReference);
        }
        keyring::Entry::new(&self.service, reference.as_str())
            .map_err(|error| map_keyring_error(&error))
    }
}

#[async_trait]
impl SecretStore for KeyringSecretStore {
    fn availability(&self) -> SecretStoreAvailability {
        SecretStoreAvailability::Available
    }

    async fn put(&self, secret: SecretValue) -> Result<SecretRef, SecretError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let sequence = NEXT_KEYRING_SECRET_ID.fetch_add(1, Ordering::Relaxed);
        let reference = SecretRef::new(format!("key-{timestamp}-{sequence}"));
        self.entry(&reference)?
            .set_password(secret.as_str())
            .map_err(|error| map_keyring_error(&error))?;
        Ok(reference)
    }

    async fn get(&self, reference: &SecretRef) -> Result<SecretValue, SecretError> {
        let value = self
            .entry(reference)?
            .get_password()
            .map_err(|error| map_keyring_error(&error))?;
        Ok(SecretValue::new(value))
    }

    async fn delete(&self, reference: &SecretRef) -> Result<(), SecretError> {
        self.entry(reference)?
            .delete_credential()
            .map_err(|error| map_keyring_error(&error))
    }
}

/// SecretStore implementation that keeps encrypted secret payloads in the
/// application SQLite database while keeping the wrapping key in the OS
/// credential manager. Existing non-SQLite references remain readable through
/// the delegated backend, so switching storage does not invalidate profiles.
pub struct SqliteSecretStore {
    pool: SqlitePool,
    wrapping_key: aead::LessSafeKey,
    delegated: Arc<dyn SecretStore>,
}

const SQLITE_SECRET_PREFIX: &str = "sqlite-secret-";
const WRAPPING_KEY_REF: &str = "sqlite-secret-wrapping-key-ref";
const NONCE_LEN: usize = 12;

impl SqliteSecretStore {
    /// Opens the encrypted SQLite store using the platform keyring backend.
    ///
    /// # Errors
    ///
    /// Returns a stable secret-store error when SQLite or the wrapping key
    /// cannot be initialized.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, SecretError> {
        let delegated = Arc::new(KeyringSecretStore::new()?) as Arc<dyn SecretStore>;
        Self::open_with_backend(path, delegated).await
    }

    /// Opens the store with an injected backend, primarily for deterministic
    /// tests and platform adapters.
    pub async fn open_with_backend(
        path: impl AsRef<Path>,
        delegated: Arc<dyn SecretStore>,
    ) -> Result<Self, SecretError> {
        if delegated.availability() == SecretStoreAvailability::Unavailable {
            return Err(SecretError::Unavailable);
        }
        let options = SqliteConnectOptions::new()
            .filename(path.as_ref())
            .create_if_missing(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|_| SecretError::BackendFailure)?;
        let wrapping_key =
            match sqlx::query("SELECT value FROM secret_store_metadata WHERE key = ?")
                .bind(WRAPPING_KEY_REF)
                .fetch_optional(&pool)
                .await
                .map_err(|_| SecretError::BackendFailure)?
            {
                Some(row) => {
                    let reference: String = row
                        .try_get("value")
                        .map_err(|_| SecretError::BackendFailure)?;
                    let value = delegated
                        .get(&SecretRef::new(reference))
                        .await
                        .map_err(|_| SecretError::BackendFailure)?;
                    decode_wrapping_key(value.as_str())?
                }
                None => {
                    let mut bytes = [0_u8; 32];
                    SystemRandom::new()
                        .fill(&mut bytes)
                        .map_err(|_| SecretError::BackendFailure)?;
                    let encoded = BASE64.encode(bytes);
                    let reference = delegated
                        .put(SecretValue::new(encoded))
                        .await
                        .map_err(|_| SecretError::BackendFailure)?;
                    if sqlx::query("INSERT INTO secret_store_metadata (key, value) VALUES (?, ?)")
                        .bind(WRAPPING_KEY_REF)
                        .bind(reference.as_str())
                        .execute(&pool)
                        .await
                        .is_err()
                    {
                        let _ = delegated.delete(&reference).await;
                        return Err(SecretError::BackendFailure);
                    }
                    bytes
                }
            };
        let unbound = aead::UnboundKey::new(&aead::AES_256_GCM, &wrapping_key)
            .map_err(|_| SecretError::BackendFailure)?;
        Ok(Self {
            pool,
            wrapping_key: aead::LessSafeKey::new(unbound),
            delegated,
        })
    }

    fn uses_sqlite(reference: &SecretRef) -> bool {
        reference.as_str().starts_with(SQLITE_SECRET_PREFIX)
    }
}

#[async_trait]
impl SecretStore for SqliteSecretStore {
    fn availability(&self) -> SecretStoreAvailability {
        self.delegated.availability()
    }

    async fn put(&self, secret: SecretValue) -> Result<SecretRef, SecretError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let sequence = NEXT_KEYRING_SECRET_ID.fetch_add(1, Ordering::Relaxed);
        let reference = SecretRef::new(format!("{SQLITE_SECRET_PREFIX}{timestamp}-{sequence}"));
        let mut nonce_bytes = [0_u8; NONCE_LEN];
        SystemRandom::new()
            .fill(&mut nonce_bytes)
            .map_err(|_| SecretError::BackendFailure)?;
        let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);
        let mut ciphertext = secret.as_str().as_bytes().to_vec();
        self.wrapping_key
            .seal_in_place_append_tag(
                nonce,
                aead::Aad::from(reference.as_str().as_bytes()),
                &mut ciphertext,
            )
            .map_err(|_| SecretError::BackendFailure)?;
        let timestamp = timestamp.to_string();
        sqlx::query(
            "INSERT INTO encrypted_secrets
                (id, ciphertext, nonce, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(reference.as_str())
        .bind(ciphertext)
        .bind(nonce_bytes.to_vec())
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&self.pool)
        .await
        .map_err(|_| SecretError::BackendFailure)?;
        Ok(reference)
    }

    async fn get(&self, reference: &SecretRef) -> Result<SecretValue, SecretError> {
        if !Self::uses_sqlite(reference) {
            return self.delegated.get(reference).await;
        }
        let row = sqlx::query("SELECT ciphertext, nonce FROM encrypted_secrets WHERE id = ?")
            .bind(reference.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| SecretError::BackendFailure)?
            .ok_or(SecretError::NotFound)?;
        let mut ciphertext: Vec<u8> = row
            .try_get("ciphertext")
            .map_err(|_| SecretError::BackendFailure)?;
        let nonce: Vec<u8> = row
            .try_get("nonce")
            .map_err(|_| SecretError::BackendFailure)?;
        let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| SecretError::BackendFailure)?;
        let plaintext = self
            .wrapping_key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(reference.as_str().as_bytes()),
                &mut ciphertext,
            )
            .map_err(|_| SecretError::BackendFailure)?;
        let value =
            String::from_utf8(plaintext.to_vec()).map_err(|_| SecretError::BackendFailure)?;
        Ok(SecretValue::new(value))
    }

    async fn delete(&self, reference: &SecretRef) -> Result<(), SecretError> {
        if !Self::uses_sqlite(reference) {
            return self.delegated.delete(reference).await;
        }
        let result = sqlx::query("DELETE FROM encrypted_secrets WHERE id = ?")
            .bind(reference.as_str())
            .execute(&self.pool)
            .await
            .map_err(|_| SecretError::BackendFailure)?;
        if result.rows_affected() == 0 {
            return Err(SecretError::NotFound);
        }
        Ok(())
    }
}

fn decode_wrapping_key(value: &str) -> Result<[u8; 32], SecretError> {
    let bytes = BASE64
        .decode(value)
        .map_err(|_| SecretError::BackendFailure)?;
    bytes.try_into().map_err(|_| SecretError::BackendFailure)
}

fn map_keyring_error(error: &keyring::Error) -> SecretError {
    match error {
        keyring::Error::NoEntry => SecretError::NotFound,
        keyring::Error::NoDefaultStore => SecretError::Unavailable,
        _ => SecretError::BackendFailure,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        KeyringSecretStore, MemorySecretStore, SecretError, SecretStore, SecretStoreAvailability,
        SecretValue, SqliteSecretStore,
    };
    use sqlx::{
        sqlite::{SqliteConnectOptions, SqlitePoolOptions},
        Row,
    };
    use std::{path::PathBuf, sync::Arc};

    #[tokio::test]
    async fn stores_and_deletes_without_exposing_the_value_in_debug() {
        let store = MemorySecretStore::new();
        let secret = SecretValue::new("sk-test-only-secret");
        let reference = store.put(secret).await.expect("put should work");

        assert_eq!(
            store
                .get(&reference)
                .await
                .expect("get should work")
                .as_str(),
            "sk-test-only-secret"
        );
        assert!(!format!("{reference:?}").contains("sk-test-only-secret"));
        store.delete(&reference).await.expect("delete should work");
        assert!(matches!(
            store.get(&reference).await,
            Err(SecretError::NotFound)
        ));
    }

    #[tokio::test]
    async fn unavailable_store_never_accepts_a_secret() {
        let store = MemorySecretStore::with_availability(SecretStoreAvailability::Unavailable);
        assert_eq!(store.availability(), SecretStoreAvailability::Unavailable);
        assert_eq!(
            store.put(SecretValue::new("secret")).await,
            Err(SecretError::Unavailable)
        );
    }

    #[tokio::test]
    async fn sqlite_store_persists_ciphertext_and_round_trips_through_reopen() {
        let path = std::env::temp_dir().join(format!(
            "batch-code-analyzer-secret-store-{}.db",
            std::process::id()
        ));
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("sqlite should open");
        sqlx::query(
            "CREATE TABLE encrypted_secrets (
                id TEXT PRIMARY KEY, ciphertext BLOB NOT NULL, nonce BLOB NOT NULL,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE secret_store_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let backend = Arc::new(MemorySecretStore::new());
        let store = SqliteSecretStore::open_with_backend(&path, backend.clone())
            .await
            .expect("encrypted sqlite store should open");
        let reference = store
            .put(SecretValue::new("test-only-key-value"))
            .await
            .expect("secret should be stored");
        assert!(reference.as_str().starts_with("sqlite-secret-"));
        let row = sqlx::query("SELECT ciphertext FROM encrypted_secrets WHERE id = ?")
            .bind(reference.as_str())
            .fetch_one(&store.pool)
            .await
            .unwrap();
        let ciphertext: Vec<u8> = row.try_get("ciphertext").unwrap();
        assert_ne!(ciphertext, b"test-only-key-value");
        assert_eq!(
            store.get(&reference).await.unwrap().as_str(),
            "test-only-key-value"
        );
        store.pool.close().await;
        drop(store);

        let reopened = SqliteSecretStore::open_with_backend(&path, backend)
            .await
            .expect("encrypted sqlite store should reopen");
        assert_eq!(
            reopened.get(&reference).await.unwrap().as_str(),
            "test-only-key-value"
        );
        reopened.delete(&reference).await.unwrap();
        reopened.pool.close().await;
        drop(reopened);
        remove_database_artifacts(path);
    }

    #[test]
    fn keyring_store_uses_a_stable_application_service_name() {
        assert_eq!(
            KeyringSecretStore::DEFAULT_SERVICE,
            "com.batchcodeanalyzer.desktop"
        );
    }

    #[test]
    fn keyring_errors_map_to_stable_secret_store_codes() {
        assert_eq!(
            super::map_keyring_error(&keyring::Error::NoEntry),
            SecretError::NotFound
        );
        assert_eq!(
            super::map_keyring_error(&keyring::Error::NoDefaultStore),
            SecretError::Unavailable
        );
    }

    fn remove_database_artifacts(path: PathBuf) {
        for suffix in ["", "-wal", "-shm"] {
            let candidate = if suffix.is_empty() {
                path.clone()
            } else {
                PathBuf::from(format!("{}{}", path.display(), suffix))
            };
            let _ = std::fs::remove_file(candidate);
        }
    }
}
