//! Non-sensitive API connection profiles shared by provider adapters.

#![forbid(unsafe_code)]

use std::{fmt, time::SystemTime};

use batch_code_analyzer_secret_store::SecretRef;
use serde::{Deserialize, Serialize};
use url::Url;

/// Stable identifier for a saved API profile.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ApiProfileId(String);

impl ApiProfileId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ApiProfileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Protocols supported by this provider foundation.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApiProtocol {
    #[default]
    OpenAiResponses,
}

/// Cached model metadata. It contains no credentials or request content.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: Option<String>,
    pub owned_by: Option<String>,
}

impl ModelInfo {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: None,
            owned_by: None,
        }
    }
}

/// A persisted profile contains only safe metadata and a keychain reference.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfile {
    pub id: ApiProfileId,
    pub name: String,
    pub protocol: ApiProtocol,
    pub base_url: String,
    pub secret_ref: SecretRef,
    pub default_model: Option<String>,
    pub model_cache: Vec<ModelInfo>,
    pub model_cache_updated_at: Option<String>,
}

impl ApiProfile {
    /// Creates and validates a profile without accepting an API key.
    ///
    /// # Errors
    ///
    /// Returns a validation or secret-reference error when the name, URL, or
    /// opaque key reference is not acceptable.
    pub fn new(
        id: ApiProfileId,
        name: impl Into<String>,
        base_url: impl AsRef<str>,
        secret_ref: SecretRef,
    ) -> Result<Self, ProfileError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(ProfileError::InvalidName);
        }
        let base_url = normalize_base_url(base_url.as_ref())?;
        if secret_ref.as_str().is_empty() {
            return Err(ProfileError::MissingSecretReference);
        }
        Ok(Self {
            id,
            name,
            protocol: ApiProtocol::OpenAiResponses,
            base_url,
            secret_ref,
            default_model: None,
            model_cache: Vec::new(),
            model_cache_updated_at: None,
        })
    }

    #[must_use]
    pub fn resolve(&self) -> ResolvedApiProfile {
        ResolvedApiProfile::from(self)
    }

    /// Replaces the model cache after a successful provider list request.
    pub fn set_model_cache(&mut self, models: Vec<ModelInfo>, updated_at: SystemTime) {
        self.model_cache = models;
        self.model_cache_updated_at = Some(format_system_time(updated_at));
    }
}

/// Values required by a provider at request time, still without the secret.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedApiProfile {
    pub id: ApiProfileId,
    pub name: String,
    pub protocol: ApiProtocol,
    pub base_url: String,
    pub responses_endpoint: String,
    pub models_endpoint: String,
    pub secret_ref: SecretRef,
    pub default_model: Option<String>,
}

impl From<&ApiProfile> for ResolvedApiProfile {
    fn from(profile: &ApiProfile) -> Self {
        let base_url = profile.base_url.clone();
        let responses_endpoint = if base_url.ends_with("/responses") {
            base_url.clone()
        } else {
            format!("{base_url}/responses")
        };
        let models_endpoint = if let Some(prefix) = base_url.strip_suffix("/responses") {
            format!("{prefix}/models")
        } else {
            format!("{base_url}/models")
        };
        Self {
            id: profile.id.clone(),
            name: profile.name.clone(),
            protocol: profile.protocol,
            base_url,
            responses_endpoint,
            models_endpoint,
            secret_ref: profile.secret_ref.clone(),
            default_model: profile.default_model.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileError {
    InvalidName,
    InvalidBaseUrl,
    UnsupportedScheme,
    UrlContainsCredentials,
    MissingSecretReference,
}

impl ProfileError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidName | Self::InvalidBaseUrl | Self::UnsupportedScheme => {
                "validation_invalid_value"
            }
            Self::UrlContainsCredentials | Self::MissingSecretReference => {
                "security_invalid_secret_reference"
            }
        }
    }
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProfileError {}

fn normalize_base_url(value: &str) -> Result<String, ProfileError> {
    let trimmed = value.trim().trim_end_matches('/');
    let parsed = Url::parse(trimmed).map_err(|_| ProfileError::InvalidBaseUrl)?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ProfileError::UnsupportedScheme);
    }
    if parsed.username() != "" || parsed.password().is_some() {
        return Err(ProfileError::UrlContainsCredentials);
    }
    if parsed.query().is_some() || parsed.fragment().is_some() || parsed.host_str().is_none() {
        return Err(ProfileError::InvalidBaseUrl);
    }
    Ok(trimmed.to_owned())
}

fn format_system_time(value: SystemTime) -> String {
    value.duration_since(SystemTime::UNIX_EPOCH).map_or_else(
        |_| "0".to_owned(),
        |duration| duration.as_secs().to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::{ApiProfile, ApiProfileId, ProfileError};
    use batch_code_analyzer_secret_store::SecretRef;

    #[test]
    fn normalizes_profile_endpoints_without_duplicate_paths() {
        let profile = ApiProfile::new(
            ApiProfileId::new("profile-1"),
            "Local",
            "https://example.test/v1/",
            SecretRef::new("ref-1"),
        )
        .expect("valid profile");
        let resolved = profile.resolve();
        assert_eq!(
            resolved.responses_endpoint,
            "https://example.test/v1/responses"
        );
        assert_eq!(resolved.models_endpoint, "https://example.test/v1/models");

        let endpoint_profile = ApiProfile::new(
            ApiProfileId::new("profile-2"),
            "Endpoint",
            "https://example.test/v1/responses",
            SecretRef::new("ref-2"),
        )
        .expect("valid endpoint");
        assert_eq!(
            endpoint_profile.resolve().responses_endpoint,
            "https://example.test/v1/responses"
        );
        assert_eq!(
            endpoint_profile.resolve().models_endpoint,
            "https://example.test/v1/models"
        );
    }

    #[test]
    fn rejects_credentials_and_non_http_urls() {
        assert_eq!(
            ApiProfile::new(
                ApiProfileId::new("x"),
                "x",
                "ftp://example.test",
                SecretRef::new("r")
            ),
            Err(ProfileError::UnsupportedScheme)
        );
        assert_eq!(
            ApiProfile::new(
                ApiProfileId::new("x"),
                "x",
                "https://user:pass@example.test",
                SecretRef::new("r")
            ),
            Err(ProfileError::UrlContainsCredentials)
        );
    }
}
