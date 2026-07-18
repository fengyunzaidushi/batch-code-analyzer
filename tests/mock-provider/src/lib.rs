//! Deterministic local HTTP fixtures for Responses API tests.

#![forbid(unsafe_code)]

use std::{io, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
};

/// A response scenario that never contacts a real model service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MockScenario {
    Normal,
    MissingTokenFields,
    Delayed,
    RateLimited { retry_after_seconds: u64 },
    Unauthorized,
    ForbiddenModel,
    ForbiddenAccount,
    ForbiddenContent,
    ServerError { status: u16 },
    InvalidJson,
    Disconnect,
}

/// Configuration shared by every request accepted by a mock server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MockServerConfig {
    pub scenario: MockScenario,
    pub delay: Duration,
    pub response_id: String,
    pub model: String,
    pub output_text: String,
}

impl MockServerConfig {
    #[must_use]
    pub fn new(scenario: MockScenario) -> Self {
        Self {
            scenario,
            delay: Duration::ZERO,
            response_id: "mock-response-001".into(),
            model: "mock-model".into(),
            output_text: "mock output".into(),
        }
    }

    #[must_use]
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    #[must_use]
    pub fn with_response_id(mut self, response_id: impl Into<String>) -> Self {
        self.response_id = response_id.into();
        self
    }

    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    #[must_use]
    pub fn with_output_text(mut self, output_text: impl Into<String>) -> Self {
        self.output_text = output_text.into();
        self
    }
}

/// A local HTTP server with a stable endpoint for one mock scenario.
pub struct MockResponsesServer {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

/// Alias used by integration tests that refer to the fixture as a provider.
pub type MockProviderServer = MockResponsesServer;

impl MockResponsesServer {
    /// Binds an ephemeral loopback port and starts accepting concurrent requests.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the loopback listener cannot be created.
    pub async fn start(config: MockServerConfig) -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        let (shutdown_sender, mut shutdown_receiver) = oneshot::channel();
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_receiver => break,
                    result = listener.accept() => {
                        let Ok((stream, _)) = result else { break };
                        let request_config = config.clone();
                        tokio::spawn(async move {
                            let _ = handle_connection(stream, request_config).await;
                        });
                    }
                }
            }
        });
        Ok(Self {
            base_url: format!("http://{address}/v1"),
            shutdown: Some(shutdown_sender),
            task: Some(task),
        })
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    #[must_use]
    pub fn authority(&self) -> &str {
        self.base_url
            .strip_prefix("http://")
            .and_then(|value| value.split('/').next())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn responses_url(&self) -> String {
        format!("{}/responses", self.base_url)
    }

    #[must_use]
    pub fn models_url(&self) -> String {
        format!("{}/models", self.base_url)
    }

    /// Stops the accept loop and waits for it to finish.
    pub async fn shutdown(mut self) {
        if let Some(sender) = self.shutdown.take() {
            let _ = sender.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

impl Drop for MockResponsesServer {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn handle_connection(mut stream: TcpStream, config: MockServerConfig) -> io::Result<()> {
    let request = read_request(&mut stream).await?;
    if config.delay > Duration::ZERO {
        tokio::time::sleep(config.delay).await;
    }
    if matches!(config.scenario, MockScenario::Disconnect) {
        return Ok(());
    }

    let path = request_path(&request);
    let (status, headers, body) = response_for(&config, &path);
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{headers}\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(&body).await
}

async fn read_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut headers = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 2048];
    loop {
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            return Ok(headers);
        }
        headers.extend_from_slice(&chunk[..count]);
        if headers.windows(4).any(|window| window == b"\r\n\r\n") || headers.len() > 64 * 1024 {
            break;
        }
    }
    let content_length = header_value(&headers, "content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let header_end = headers
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map_or(headers.len(), |position| position + 4);
    let already_read = headers.len().saturating_sub(header_end);
    let mut remaining = content_length.saturating_sub(already_read);
    while remaining > 0 {
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            break;
        }
        remaining = remaining.saturating_sub(count);
    }
    Ok(headers)
}

fn request_path(request: &[u8]) -> String {
    let line_end = request
        .windows(2)
        .position(|window| window == b"\r\n")
        .unwrap_or(request.len());
    let line = String::from_utf8_lossy(&request[..line_end]);
    line.split_whitespace().nth(1).unwrap_or("/").to_owned()
}

fn header_value<'a>(request: &'a [u8], wanted: &str) -> Option<&'a str> {
    let text = std::str::from_utf8(request).ok()?;
    text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case(wanted)
            .then_some(value.trim())
    })
}

fn response_for(config: &MockServerConfig, path: &str) -> (&'static str, String, Vec<u8>) {
    if path.ends_with("/models")
        && matches!(
            config.scenario,
            MockScenario::Normal | MockScenario::MissingTokenFields | MockScenario::Delayed
        )
    {
        let body = format!(
            r#"{{"object":"list","data":[{{"id":"{}","owned_by":"mock"}}]}}"#,
            config.model
        );
        return ("200 OK", String::new(), body.into_bytes());
    }

    match &config.scenario {
        MockScenario::Normal | MockScenario::Delayed => (
            "200 OK",
            String::new(),
            success_body(config, true).into_bytes(),
        ),
        MockScenario::MissingTokenFields => (
            "200 OK",
            String::new(),
            success_body(config, false).into_bytes(),
        ),
        MockScenario::RateLimited {
            retry_after_seconds,
        } => (
            "429 Too Many Requests",
            format!("Retry-After: {retry_after_seconds}\r\n"),
            error_body("rate_limit", "request was rate limited").into_bytes(),
        ),
        MockScenario::Unauthorized => (
            "401 Unauthorized",
            String::new(),
            error_body("invalid_api_key", "mock authentication failure").into_bytes(),
        ),
        MockScenario::ForbiddenModel => (
            "403 Forbidden",
            String::new(),
            error_body("model_not_found", "mock model permission failure").into_bytes(),
        ),
        MockScenario::ForbiddenAccount => (
            "403 Forbidden",
            String::new(),
            error_body(
                "account_permission_denied",
                "mock account permission failure",
            )
            .into_bytes(),
        ),
        MockScenario::ForbiddenContent => (
            "403 Forbidden",
            String::new(),
            error_body("content_policy", "mock content policy failure").into_bytes(),
        ),
        MockScenario::ServerError { status } => (
            server_status_line(*status),
            String::new(),
            error_body("server_error", "mock server failure").into_bytes(),
        ),
        MockScenario::InvalidJson => ("200 OK", String::new(), b"{not-json".to_vec()),
        MockScenario::Disconnect => ("204 No Content", String::new(), Vec::new()),
    }
}

fn success_body(config: &MockServerConfig, include_usage: bool) -> String {
    if include_usage {
        format!(
            r#"{{"id":"{}","object":"response","model":"{}","output_text":"{}","usage":{{"input_tokens":12,"output_tokens":8,"total_tokens":20}}}}"#,
            config.response_id, config.model, config.output_text
        )
    } else {
        format!(
            r#"{{"id":"{}","object":"response","model":"{}","output_text":"{}"}}"#,
            config.response_id, config.model, config.output_text
        )
    }
}

fn error_body(code: &str, message: &str) -> String {
    format!(r#"{{"error":{{"code":"{code}","message":"{message}"}}}}"#)
}

fn server_status_line(status: u16) -> &'static str {
    match status {
        502 => "502 Bad Gateway",
        503 => "503 Service Unavailable",
        _ => "500 Internal Server Error",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };

    use super::{MockResponsesServer, MockScenario, MockServerConfig};

    #[tokio::test]
    async fn serves_a_deterministic_response_and_can_be_stopped() {
        let server = MockResponsesServer::start(
            MockServerConfig::new(MockScenario::Normal).with_response_id("response-test"),
        )
        .await
        .expect("server should bind");
        let mut stream = TcpStream::connect(server.authority())
            .await
            .expect("server should accept");
        stream
            .write_all(b"GET /v1/models HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .expect("request should write");
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .expect("response should read");
        assert!(String::from_utf8_lossy(&response).contains("mock-model"));
        server.shutdown().await;
    }

    #[test]
    fn delayed_config_is_explicit() {
        assert_eq!(
            MockServerConfig::new(MockScenario::Delayed)
                .with_delay(Duration::from_millis(25))
                .delay,
            Duration::from_millis(25)
        );
    }
}
