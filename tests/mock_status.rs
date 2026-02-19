mod common;

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

// ── is_active ──

#[tokio::test]
async fn is_active_ok_on_200() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/active"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.is_active().await.unwrap();
}

#[tokio::test]
async fn is_active_ok_on_204() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/active"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    client.is_active().await.unwrap();
}

#[tokio::test]
async fn is_active_unauthorized_on_401() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/active"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let err = client.is_active().await.unwrap_err();
    assert!(matches!(err, cider_api::CiderError::Unauthorized));
}

#[tokio::test]
async fn is_active_unauthorized_on_403() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/active"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let err = client.is_active().await.unwrap_err();
    assert!(matches!(err, cider_api::CiderError::Unauthorized));
}

#[tokio::test]
async fn is_active_api_error_on_unexpected_status() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/active"))
        .respond_with(ResponseTemplate::new(418))
        .mount(&server)
        .await;
    let err = client.is_active().await.unwrap_err();
    assert!(matches!(err, cider_api::CiderError::Api(_)));
}

// ── is_playing ──

#[tokio::test]
async fn is_playing_returns_true() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/is-playing"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::is_playing_json(true))
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    assert!(client.is_playing().await.unwrap());
}

#[tokio::test]
async fn is_playing_returns_false() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/is-playing"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::is_playing_json(false))
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    assert!(!client.is_playing().await.unwrap());
}

// ── now_playing ──

#[tokio::test]
async fn now_playing_returns_track() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/now-playing"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::now_playing_json())
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;

    let track = client.now_playing().await.unwrap().unwrap();
    assert_eq!(track.name, "Never Be Like You");
    assert_eq!(track.artist_name, "Flume");
    assert_eq!(track.song_id(), Some("1719861213"));
    assert_eq!(
        track.artwork_url(300),
        "https://example.com/300x300bb.jpg"
    );
}

#[tokio::test]
async fn now_playing_returns_none_on_404() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/now-playing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    assert!(client.now_playing().await.unwrap().is_none());
}

#[tokio::test]
async fn now_playing_returns_none_on_204() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/now-playing"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    assert!(client.now_playing().await.unwrap().is_none());
}

#[tokio::test]
async fn now_playing_returns_none_on_unparseable_json() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/now-playing"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"garbage": true}"#)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    assert!(client.now_playing().await.unwrap().is_none());
}
