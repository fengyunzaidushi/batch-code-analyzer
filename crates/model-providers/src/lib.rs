//! `OpenAI` Responses API adapter with stable, sanitized provider errors.

#![forbid(unsafe_code)]

use std::{
    fmt,
    sync::Arc,
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
use batch_code_analyzer_api_profiles::{ModelInfo, ResolvedApiProfile};
use batch_code_analyzer_secret_store::{SecretError, SecretStore};
use reqwest::{header, Client, Method, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

/// Request data sent to a provider. It contains source text in memory only and
/// is never included in provider errors or log records by this crate.
#[derive(Clone)]
pub struct ProviderRequest {
    pub profile: ResolvedApiProfile,
    pub model: String,
    pub input: String,
    pub instructions: Option<String>,
    pub max_output_tokens: Option<u32>,
    pub timeout: Option<Duration>,
}

impl fmt::Debug for ProviderRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderRequest")
            .field("profile", &self.profile)
            .field("model", &self.model)
            .field("input", &"[REDACTED]")
            .field(
                "instructions",
                &self.instructions.as_ref().map(|_| "[REDACTED]"),
            )
            .field("max_output_tokens", &self.max_output_tokens)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl ProviderRequest {
    #[must_use]
    pub fn new(
        profile: ResolvedApiProfile,
        model: impl Into<String>,
        input: impl Into<String>,
    ) -> Self {
        Self {
            profile,
            model: model.into(),
            input: input.into(),
            instructions: None,
            max_output_tokens: None,
            timeout: None,
        }
    }

    #[must_use]
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    #[must_use]
    pub fn with_max_output_tokens(mut self, value: u32) -> Self {
        self.max_output_tokens = Some(value);
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

#[derive(Clone, Default, Eq, PartialEq)]
pub struct ProviderResponse {
    pub response_id: Option<String>,
    pub model: Option<String>,
    pub output_text: String,
    pub usage: TokenUsage,
}

impl fmt::Debug for ProviderResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderResponse")
            .field("response_id", &self.response_id)
            .field("model", &self.model)
            .field("output_text", &"[REDACTED]")
            .field("output_length", &self.output_text.len())
            .field("usage", &self.usage)
            .finish()
    }
}

/// Stable error categories returned by model adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderError {
    ConnectionFailed,
    Timeout,
    RateLimited { retry_after_seconds: Option<u64> },
    ServerError { status: u16 },
    AuthenticationFailed { status: u16 },
    PermissionDenied { status: u16 },
    ModelUnavailable { status: u16, model: Option<String> },
    ContentRejected { status: u16 },
    InvalidRequest { status: u16 },
    InvalidResponse,
    Cancelled,
    InterruptedUnknown,
    SecretStoreUnavailable,
}

impl ProviderError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ConnectionFailed => "provider_connection_failed",
            Self::Timeout => "provider_timeout",
            Self::RateLimited { .. } => "provider_rate_limited",
            Self::ServerError { .. } => "provider_server_error",
            Self::AuthenticationFailed { .. } => "provider_authentication_failed",
            Self::PermissionDenied { .. } => "provider_permission_denied",
            Self::ModelUnavailable { .. } => "provider_model_unavailable",
            Self::ContentRejected { .. } => "provider_content_rejected",
            Self::InvalidRequest { .. } => "provider_invalid_request",
            Self::InvalidResponse => "provider_invalid_response",
            Self::Cancelled => "provider_cancelled",
            Self::InterruptedUnknown => "provider_interrupted_unknown",
            Self::SecretStoreUnavailable => "security_secret_store_unavailable",
        }
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        matches!(
            self,
            Self::ConnectionFailed
                | Self::Timeout
                | Self::RateLimited { .. }
                | Self::ServerError { .. }
                | Self::InvalidResponse
        )
    }

    #[must_use]
    pub const fn switch_profile(&self) -> bool {
        matches!(
            self,
            Self::ConnectionFailed
                | Self::Timeout
                | Self::RateLimited { .. }
                | Self::ServerError { .. }
                | Self::AuthenticationFailed { .. }
                | Self::PermissionDenied { .. }
                | Self::ModelUnavailable { .. }
                | Self::InvalidResponse
        )
    }

    #[must_use]
    pub const fn retry_after_seconds(&self) -> Option<u64> {
        match self {
            Self::RateLimited {
                retry_after_seconds,
            } => *retry_after_seconds,
            _ => None,
        }
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProviderError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ErrorClassification {
    pub code: &'static str,
    pub retryable: bool,
    pub switch_profile: bool,
}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn list_models(
        &self,
        profile: &ResolvedApiProfile,
    ) -> Result<Vec<ModelInfo>, ProviderError>;

    async fn execute(
        &self,
        request: ProviderRequest,
        cancel: CancellationToken,
    ) -> Result<ProviderResponse, ProviderError>;

    fn classify_error(&self, error: &ProviderError) -> ErrorClassification;
}

#[derive(Clone)]
pub struct OpenAiResponsesProvider {
    client: Client,
    secrets: Arc<dyn SecretStore>,
    default_timeout: Duration,
}

impl OpenAiResponsesProvider {
    /// Creates an adapter using a shared secret backend.
    ///
    /// # Errors
    ///
    /// Returns `provider_connection_failed` if the HTTP client cannot be
    /// initialized.
    pub fn new(secrets: Arc<dyn SecretStore>) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .build()
            .map_err(|_| ProviderError::ConnectionFailed)?;
        Ok(Self {
            client,
            secrets,
            default_timeout: Duration::from_mins(2),
        })
    }

    /// Creates an adapter with an already configured HTTP client.
    pub fn with_client(
        client: Client,
        secrets: Arc<dyn SecretStore>,
        default_timeout: Duration,
    ) -> Self {
        Self {
            client,
            secrets,
            default_timeout,
        }
    }

    async fn secret(&self, profile: &ResolvedApiProfile) -> Result<String, ProviderError> {
        self.secrets
            .get(&profile.secret_ref)
            .await
            .map(|secret| secret.as_str().to_owned())
            .map_err(map_secret_error)
    }

    async fn request(
        &self,
        method: Method,
        endpoint: &str,
        key: &str,
        body: Option<Value>,
        timeout: Duration,
        cancel: Option<&CancellationToken>,
    ) -> Result<Response, ProviderError> {
        let mut builder = self
            .client
            .request(method, endpoint)
            .header(header::AUTHORIZATION, format!("Bearer {key}"))
            .header(header::ACCEPT, "application/json");
        if let Some(body) = body {
            builder = builder
                .header(header::CONTENT_TYPE, "application/json")
                .json(&body);
        }
        let future = builder.send();
        let response = if let Some(cancel) = cancel {
            tokio::select! {
                () = cancel.cancelled() => return Err(ProviderError::Cancelled),
                result = tokio::time::timeout(timeout, future) => map_timeout(result)?,
            }
        } else {
            tokio::time::timeout(timeout, future)
                .await
                .map_err(|_| ProviderError::Timeout)?
                .map_err(|_| ProviderError::ConnectionFailed)?
        };
        Ok(response)
    }

    async fn read_body(
        &self,
        response: Response,
        timeout: Duration,
        cancel: Option<&CancellationToken>,
    ) -> Result<(StatusCode, reqwest::header::HeaderMap, Vec<u8>), ProviderError> {
        let status = response.status();
        let headers = response.headers().clone();
        let future = response.bytes();
        let bytes = if let Some(cancel) = cancel {
            tokio::select! {
                () = cancel.cancelled() => return Err(ProviderError::Cancelled),
                result = tokio::time::timeout(timeout, future) => map_body_timeout(result)?,
            }
        } else {
            tokio::time::timeout(timeout, future)
                .await
                .map_err(|_| ProviderError::Timeout)?
                .map_err(|_| ProviderError::ConnectionFailed)?
        };
        Ok((status, headers, bytes.to_vec()))
    }
}

#[async_trait]
impl ModelProvider for OpenAiResponsesProvider {
    async fn list_models(
        &self,
        profile: &ResolvedApiProfile,
    ) -> Result<Vec<ModelInfo>, ProviderError> {
        let key = self.secret(profile).await?;
        let response = self
            .request(
                Method::GET,
                &profile.models_endpoint,
                &key,
                None,
                self.default_timeout,
                None,
            )
            .await?;
        let (status, _, body) = self.read_body(response, self.default_timeout, None).await?;
        if !status.is_success() {
            return Err(map_http_error(status, &body, None));
        }
        let payload: ModelsPayload =
            serde_json::from_slice(&body).map_err(|_| ProviderError::InvalidResponse)?;
        Ok(payload
            .data
            .into_iter()
            .map(|model| ModelInfo {
                id: model.id,
                display_name: model.display_name,
                owned_by: model.owned_by,
            })
            .collect())
    }

    async fn execute(
        &self,
        request: ProviderRequest,
        cancel: CancellationToken,
    ) -> Result<ProviderResponse, ProviderError> {
        let key = self.secret(&request.profile).await?;
        let body = serde_json::json!({
            "model": request.model,
            "input": request.input,
            // Responses treats this as an optional system instruction. Send an
            // explicit empty value when the caller has no instruction so a
            // provider cannot substitute an implicit default instruction.
            "instructions": request.instructions.as_deref().unwrap_or_default(),
            "max_output_tokens": request.max_output_tokens,
        });
        let timeout = request.timeout.unwrap_or(self.default_timeout);
        let response = self
            .request(
                Method::POST,
                &request.profile.responses_endpoint,
                &key,
                Some(body),
                timeout,
                Some(&cancel),
            )
            .await?;
        let (status, headers, bytes) = self.read_body(response, timeout, Some(&cancel)).await?;
        if !status.is_success() {
            return Err(map_http_error(status, &bytes, Some(&headers)));
        }
        let payload: ResponsesPayload =
            serde_json::from_slice(&bytes).map_err(|_| ProviderError::InvalidResponse)?;
        let output_text = payload
            .output_text
            .or_else(|| extract_output_text(payload.output.as_deref()))
            .ok_or(ProviderError::InvalidResponse)?;
        Ok(ProviderResponse {
            response_id: payload.id,
            model: payload.model,
            output_text,
            usage: payload
                .usage
                .as_ref()
                .map_or_else(TokenUsage::default, map_usage),
        })
    }

    fn classify_error(&self, error: &ProviderError) -> ErrorClassification {
        ErrorClassification {
            code: error.code(),
            retryable: error.retryable(),
            switch_profile: error.switch_profile(),
        }
    }
}

fn map_secret_error(error: SecretError) -> ProviderError {
    match error {
        SecretError::Unavailable
        | SecretError::BackendFailure
        | SecretError::NotFound
        | SecretError::InvalidReference => ProviderError::SecretStoreUnavailable,
    }
}

fn map_timeout<T>(
    result: Result<Result<T, reqwest::Error>, tokio::time::error::Elapsed>,
) -> Result<T, ProviderError> {
    result
        .map_err(|_| ProviderError::Timeout)?
        .map_err(|_| ProviderError::ConnectionFailed)
}

fn map_body_timeout<T>(
    result: Result<Result<T, reqwest::Error>, tokio::time::error::Elapsed>,
) -> Result<T, ProviderError> {
    map_timeout(result)
}

fn map_http_error(
    status: StatusCode,
    body: &[u8],
    headers: Option<&reqwest::header::HeaderMap>,
) -> ProviderError {
    let code = status.as_u16();
    if status == StatusCode::UNAUTHORIZED {
        return ProviderError::AuthenticationFailed { status: code };
    }
    if status == StatusCode::FORBIDDEN {
        return classify_forbidden(code, body);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return ProviderError::RateLimited {
            retry_after_seconds: headers.and_then(retry_after_seconds),
        };
    }
    if status.is_server_error() {
        return ProviderError::ServerError { status: code };
    }
    if status == StatusCode::NOT_FOUND {
        return ProviderError::ModelUnavailable {
            status: code,
            model: None,
        };
    }
    ProviderError::InvalidRequest { status: code }
}

fn classify_forbidden(status: u16, body: &[u8]) -> ProviderError {
    let text = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| value.get("error").cloned().or(Some(value)))
        .map(|value| value.to_string().to_ascii_lowercase())
        .unwrap_or_default();
    if ["content", "safety", "moderation", "policy"]
        .iter()
        .any(|term| text.contains(term))
    {
        ProviderError::ContentRejected { status }
    } else if ["model", "does_not_have_access", "model_not_found"]
        .iter()
        .any(|term| text.contains(term))
    {
        ProviderError::ModelUnavailable {
            status,
            model: None,
        }
    } else {
        ProviderError::PermissionDenied { status }
    }
}

fn retry_after_seconds(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    let value = headers.get(header::RETRY_AFTER)?.to_str().ok()?;
    if let Ok(seconds) = value.trim().parse::<u64>() {
        return Some(seconds);
    }
    let date = httpdate::parse_http_date(value).ok()?;
    date.duration_since(SystemTime::now())
        .ok()
        .map(|duration| duration.as_secs())
}

fn extract_output_text(output: Option<&[OutputItem]>) -> Option<String> {
    let text = output?
        .iter()
        .flat_map(|item| item.content.iter())
        .filter_map(|content| content.text.as_deref())
        .collect::<Vec<_>>()
        .join("");
    (!text.is_empty()).then_some(text)
}

fn map_usage(usage: &UsagePayload) -> TokenUsage {
    TokenUsage {
        input_tokens: usage.input,
        output_tokens: usage.output,
        total_tokens: usage.total,
    }
}

#[derive(Debug, Deserialize)]
struct ModelsPayload {
    #[serde(default)]
    data: Vec<ModelPayload>,
}

#[derive(Debug, Deserialize)]
struct ModelPayload {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    owned_by: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponsesPayload {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    output_text: Option<String>,
    #[serde(default)]
    output: Option<Vec<OutputItem>>,
    #[serde(default)]
    usage: Option<UsagePayload>,
}

#[derive(Debug, Deserialize)]
struct OutputItem {
    #[serde(default)]
    content: Vec<ContentItem>,
}

#[derive(Debug, Deserialize)]
struct ContentItem {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsagePayload {
    #[serde(default)]
    #[serde(rename = "input_tokens")]
    input: Option<u64>,
    #[serde(default)]
    #[serde(rename = "output_tokens")]
    output: Option<u64>,
    #[serde(default)]
    #[serde(rename = "total_tokens")]
    total: Option<u64>,
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use batch_code_analyzer_api_profiles::{ApiProfile, ApiProfileId};
    use batch_code_analyzer_secret_store::{MemorySecretStore, SecretStore, SecretValue};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use tokio_util::sync::CancellationToken;

    use super::{ModelProvider, OpenAiResponsesProvider, ProviderError, ProviderRequest};

    async fn server(response: String, wait_for_request: bool) -> String {
        server_with_status("200 OK", "", response, wait_for_request).await
    }

    async fn server_with_status(
        status: &str,
        extra_headers: &str,
        response: String,
        wait_for_request: bool,
    ) -> String {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("bind");
        let address = listener.local_addr().expect("address");
        let status = status.to_owned();
        let extra_headers = extra_headers.to_owned();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut request = [0; 8192];
            let _ = stream.read(&mut request).await;
            if wait_for_request {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            let bytes = response.as_bytes();
            let header = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\n{extra_headers}\r\n",
                bytes.len(),
            );
            let _ = stream.write_all(header.as_bytes()).await;
            let _ = stream.write_all(bytes).await;
        });
        format!("http://{address}/v1")
    }

    async fn profile(base_url: String) -> (OpenAiResponsesProvider, ProviderRequest) {
        let store = Arc::new(MemorySecretStore::new());
        let reference = store
            .put(SecretValue::new("sk-test-key-never-log"))
            .await
            .expect("secret");
        let profile = ApiProfile::new(ApiProfileId::new("profile-1"), "Mock", base_url, reference)
            .expect("profile");
        let request = ProviderRequest::new(profile.resolve(), "gpt-test", "hello")
            .with_timeout(Duration::from_millis(100));
        (
            OpenAiResponsesProvider::new(store).expect("provider"),
            request,
        )
    }

    #[tokio::test]
    async fn parses_response_when_token_usage_is_missing() {
        let base_url = server(
            r#"{"id":"resp-1","model":"gpt-test","output_text":"hello"}"#.into(),
            false,
        )
        .await;
        let (provider, request) = profile(base_url).await;
        let response = provider
            .execute(request, CancellationToken::new())
            .await
            .expect("response");
        assert_eq!(response.output_text, "hello");
        assert_eq!(response.usage.input_tokens, None);
    }

    #[tokio::test]
    async fn cancellation_returns_stable_cancelled_error() {
        let base_url = server(r#"{"output_text":"late"}"#.into(), true).await;
        let (provider, request) = profile(base_url).await;
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            provider.execute(request, cancel).await,
            Err(ProviderError::Cancelled)
        );
    }

    #[tokio::test]
    async fn maps_http_failures_and_distinguishes_forbidden_reasons() {
        let cases = [
            (
                "401 Unauthorized",
                "",
                r#"{"error":{"message":"invalid key"}}"#,
                "provider_authentication_failed",
            ),
            (
                "403 Forbidden",
                "",
                r#"{"error":{"code":"model_not_found"}}"#,
                "provider_model_unavailable",
            ),
            (
                "403 Forbidden",
                "",
                r#"{"error":{"type":"content_policy"}}"#,
                "provider_content_rejected",
            ),
            (
                "403 Forbidden",
                "",
                r#"{"error":{"code":"account_permission_denied"}}"#,
                "provider_permission_denied",
            ),
            (
                "500 Internal Server Error",
                "",
                r#"{"error":{"message":"server"}}"#,
                "provider_server_error",
            ),
        ];
        for (status, headers, body, code) in cases {
            let base_url = server_with_status(status, headers, body.into(), false).await;
            let (provider, request) = profile(base_url).await;
            let error = provider
                .execute(request, CancellationToken::new())
                .await
                .expect_err("HTTP error expected");
            assert_eq!(error.code(), code);
        }
    }

    #[tokio::test]
    async fn parses_retry_after_and_rejects_invalid_json() {
        let base_url = server_with_status(
            "429 Too Many Requests",
            "Retry-After: 7\r\n",
            r#"{"error":{"type":"rate_limit"}}"#.into(),
            false,
        )
        .await;
        let (provider, request) = profile(base_url).await;
        let error = provider
            .execute(request, CancellationToken::new())
            .await
            .expect_err("rate limit expected");
        assert_eq!(error.code(), "provider_rate_limited");
        assert_eq!(error.retry_after_seconds(), Some(7));

        let base_url = server("not-json".into(), false).await;
        let (provider, request) = profile(base_url).await;
        let error = provider
            .execute(request, CancellationToken::new())
            .await
            .expect_err("invalid response expected");
        assert_eq!(error, ProviderError::InvalidResponse);
    }

    #[tokio::test]
    async fn timeout_returns_provider_timeout() {
        let base_url = server(r#"{"output_text":"late"}"#.into(), true).await;
        let (provider, request) = profile(base_url).await;
        let error = provider
            .execute(request, CancellationToken::new())
            .await
            .expect_err("timeout expected");
        assert_eq!(error, ProviderError::Timeout);
    }

    #[test]
    fn retry_after_integer_is_parsed_and_errors_are_sanitized() {
        let error = ProviderError::RateLimited {
            retry_after_seconds: Some(10),
        };
        assert_eq!(error.code(), "provider_rate_limited");
        assert_eq!(error.retry_after_seconds(), Some(10));
        assert!(!error.to_string().contains("sk-test-key-never-log"));
    }

    #[test]
    fn request_debug_redacts_source_content() {
        let value = format!(
            "{:?}",
            ProviderRequest::new(
                batch_code_analyzer_api_profiles::ResolvedApiProfile {
                    id: batch_code_analyzer_api_profiles::ApiProfileId::new("id"),
                    name: "name".into(),
                    protocol: batch_code_analyzer_api_profiles::ApiProtocol::OpenAiResponses,
                    base_url: "https://example.test/v1".into(),
                    responses_endpoint: "https://example.test/v1/responses".into(),
                    models_endpoint: "https://example.test/v1/models".into(),
                    secret_ref: batch_code_analyzer_secret_store::SecretRef::new("ref"),
                    default_model: None,
                },
                "model",
                "sensitive source text",
            )
        );
        assert!(!value.contains("sensitive source text"));
    }
}
