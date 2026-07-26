use std::{sync::Arc, time::Duration};

use batch_code_analyzer_api_profiles::{ApiProfile, ApiProfileId};
use batch_code_analyzer_mock_provider::{MockResponsesServer, MockScenario, MockServerConfig};
use batch_code_analyzer_model_providers::{
    ModelProvider, OpenAiResponsesProvider, ProviderError, ProviderRequest,
};
use batch_code_analyzer_secret_store::{MemorySecretStore, SecretStore, SecretValue};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn normal_responses_include_id_usage_and_support_concurrent_requests() {
    let server = start(MockServerConfig::new(MockScenario::Normal)).await;
    let (provider, profile) = provider_and_profile(&server).await;
    let mut requests = JoinSet::new();
    for index in 0..12 {
        let provider = provider.clone();
        let profile = profile.clone();
        requests.spawn(async move {
            let request = ProviderRequest::new(profile, "mock-model", format!("fixture-{index}"));
            provider.execute(request, CancellationToken::new()).await
        });
    }
    while let Some(result) = requests.join_next().await {
        let response = result
            .expect("request task should join")
            .expect("mock should succeed");
        assert_eq!(response.response_id.as_deref(), Some("mock-response-001"));
        assert_eq!(response.usage.total_tokens, Some(20));
    }
    server.shutdown().await;
}

#[tokio::test]
async fn requests_without_instructions_send_an_empty_override() {
    let server = start(MockServerConfig::new(MockScenario::Normal)).await;
    let (provider, profile) = provider_and_profile(&server).await;
    provider
        .execute(
            ProviderRequest::new(profile.clone(), "mock-model", "fixture"),
            CancellationToken::new(),
        )
        .await
        .expect("mock should succeed");

    provider
        .execute(
            ProviderRequest::new(profile, "mock-model", "fixture")
                .with_instructions("caller instruction"),
            CancellationToken::new(),
        )
        .await
        .expect("mock should succeed");

    let bodies = server.request_bodies();
    assert_eq!(bodies.len(), 2);
    let body = String::from_utf8(bodies.first().expect("request body").clone())
        .expect("request body should be UTF-8");
    assert!(body.contains(r#""instructions":""#));
    assert!(!body.contains(r#""instructions":null#));
    let explicit_body = String::from_utf8(bodies.get(1).expect("explicit request body").clone())
        .expect("request body should be UTF-8");
    assert!(explicit_body.contains(r#""instructions":"caller instruction"#));
    server.shutdown().await;
}

#[tokio::test]
async fn model_listing_uses_the_same_local_server() {
    let server =
        start(MockServerConfig::new(MockScenario::Normal).with_model("mock-list-model")).await;
    let (provider, profile) = provider_and_profile(&server).await;
    let models = provider
        .list_models(&profile)
        .await
        .expect("models should load");
    assert_eq!(models[0].id, "mock-list-model");
    server.shutdown().await;
}

#[tokio::test]
async fn delayed_requests_are_deterministically_timeoutable() {
    let server =
        start(MockServerConfig::new(MockScenario::Delayed).with_delay(Duration::from_millis(150)))
            .await;
    let (provider, profile) = provider_and_profile(&server).await;
    let request = ProviderRequest::new(profile, "mock-model", "fixture")
        .with_timeout(Duration::from_millis(20));
    assert_eq!(
        provider
            .execute(request, CancellationToken::new())
            .await
            .expect_err("request should time out"),
        ProviderError::Timeout
    );
    server.shutdown().await;
}

#[tokio::test]
async fn rate_limit_preserves_retry_after() {
    let server = start(MockServerConfig::new(MockScenario::RateLimited {
        retry_after_seconds: 11,
    }))
    .await;
    let (provider, profile) = provider_and_profile(&server).await;
    let error = execute_error(&provider, profile).await;
    assert_eq!(
        error,
        ProviderError::RateLimited {
            retry_after_seconds: Some(11)
        }
    );
    server.shutdown().await;
}

#[tokio::test]
async fn authentication_and_all_forbidden_categories_are_stable() {
    let scenarios = [
        (MockScenario::Unauthorized, "provider_authentication_failed"),
        (MockScenario::ForbiddenModel, "provider_model_unavailable"),
        (MockScenario::ForbiddenAccount, "provider_permission_denied"),
        (MockScenario::ForbiddenContent, "provider_content_rejected"),
    ];
    for (scenario, code) in scenarios {
        let server = start(MockServerConfig::new(scenario)).await;
        let (provider, profile) = provider_and_profile(&server).await;
        assert_eq!(execute_error(&provider, profile).await.code(), code);
        server.shutdown().await;
    }
}

#[tokio::test]
async fn all_supported_server_error_statuses_are_retryable() {
    for status in [500, 502, 503] {
        let server = start(MockServerConfig::new(MockScenario::ServerError { status })).await;
        let (provider, profile) = provider_and_profile(&server).await;
        let error = execute_error(&provider, profile).await;
        assert_eq!(error, ProviderError::ServerError { status });
        assert!(error.retryable());
        server.shutdown().await;
    }
}

#[tokio::test]
async fn malformed_and_missing_usage_responses_are_deterministic() {
    let server = start(MockServerConfig::new(MockScenario::InvalidJson)).await;
    let (provider, profile) = provider_and_profile(&server).await;
    assert_eq!(
        execute_error(&provider, profile).await,
        ProviderError::InvalidResponse
    );
    server.shutdown().await;

    let server = start(MockServerConfig::new(MockScenario::MissingTokenFields)).await;
    let (provider, profile) = provider_and_profile(&server).await;
    let response = provider
        .execute(
            ProviderRequest::new(profile, "mock-model", "fixture"),
            CancellationToken::new(),
        )
        .await
        .expect("missing usage is still a valid response");
    assert_eq!(response.response_id.as_deref(), Some("mock-response-001"));
    assert_eq!(response.usage.total_tokens, None);
    server.shutdown().await;
}

#[tokio::test]
async fn disconnect_is_a_connection_failure_and_cancel_is_explicit() {
    let server = start(MockServerConfig::new(MockScenario::Disconnect)).await;
    let (provider, profile) = provider_and_profile(&server).await;
    let error = execute_error(&provider, profile).await;
    assert_eq!(error, ProviderError::ConnectionFailed);
    server.shutdown().await;

    let server =
        start(MockServerConfig::new(MockScenario::Delayed).with_delay(Duration::from_millis(150)))
            .await;
    let (provider, profile) = provider_and_profile(&server).await;
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert_eq!(
        provider
            .execute(
                ProviderRequest::new(profile, "mock-model", "fixture"),
                cancel
            )
            .await,
        Err(ProviderError::Cancelled)
    );
    server.shutdown().await;
}

async fn start(config: MockServerConfig) -> MockResponsesServer {
    MockResponsesServer::start(config)
        .await
        .expect("mock server should bind loopback port")
}

async fn provider_and_profile(
    server: &MockResponsesServer,
) -> (
    OpenAiResponsesProvider,
    batch_code_analyzer_api_profiles::ResolvedApiProfile,
) {
    let secrets = Arc::new(MemorySecretStore::new());
    let secret_ref = secrets
        .put(SecretValue::new("sk-local-fixture-only"))
        .await
        .expect("fixture secret should be stored");
    let profile = ApiProfile::new(
        ApiProfileId::new("mock-profile"),
        "Local Mock",
        server.base_url(),
        secret_ref,
    )
    .expect("mock profile should validate")
    .resolve();
    (
        OpenAiResponsesProvider::new(secrets).expect("provider client should build"),
        profile,
    )
}

async fn execute_error(
    provider: &OpenAiResponsesProvider,
    profile: batch_code_analyzer_api_profiles::ResolvedApiProfile,
) -> ProviderError {
    provider
        .execute(
            ProviderRequest::new(
                profile,
                "mock-model",
                "fixture source that must not be logged",
            ),
            CancellationToken::new(),
        )
        .await
        .expect_err("scenario should return a provider error")
}
