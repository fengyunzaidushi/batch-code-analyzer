//! Secret storage interfaces and a process-local implementation for tests.
//!
//! Production keychain backends can implement [`SecretStore`] without changing
//! API profile or provider code. The secret value intentionally has no `Debug`
//! or `Serialize` implementation, so it cannot accidentally cross a logging or
//! DTO boundary.

#![forbid(unsafe_code)]

use std::{collections::HashMap, fmt, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
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

#[cfg(test)]
mod tests {
    use super::{
        MemorySecretStore, SecretError, SecretStore, SecretStoreAvailability, SecretValue,
    };

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
}
