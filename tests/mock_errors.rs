mod common;

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── Auth header ──

#[tokio::test]
async fn token_is_sent_in_apitoken_header() {
    let (server, client) = common::setup_with_token("secret-123").await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/active"))
        .and(header("apptoken", "secret-123"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    client.is_active().await.unwrap();
}

#[tokio::test]
async fn token_is_sent_on_post_requests() {
    let (server, client) = common::setup_with_token("tok").await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/play"))
        .and(header("apptoken", "tok"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.play().await.unwrap();
}

// ── Error status codes ──

#[tokio::test]
async fn fire_and_forget_error_on_500() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/play"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let err = client.play().await.unwrap_err();
    assert!(matches!(err, cider_api::CiderError::Http(_)));
}

#[tokio::test]
async fn is_playing_error_on_server_error() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/is-playing"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    assert!(client.is_playing().await.is_err());
}

#[tokio::test]
async fn get_volume_error_on_server_error() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/volume"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    assert!(client.get_volume().await.is_err());
}

#[tokio::test]
async fn set_volume_error_on_server_error() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/volume"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    assert!(client.set_volume(0.5).await.is_err());
}
