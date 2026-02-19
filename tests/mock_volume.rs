mod common;

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn get_volume_returns_value() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/volume"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::volume_json(0.65))
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;

    let vol = client.get_volume().await.unwrap();
    assert!((vol - 0.65).abs() < 0.01);
}

#[tokio::test]
async fn set_volume_sends_correct_body() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/volume"))
        .and(body_json(serde_json::json!({"volume": 0.5})))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.set_volume(0.5).await.unwrap();
}

#[tokio::test]
async fn set_volume_clamps_above_1() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/volume"))
        .and(body_json(serde_json::json!({"volume": 1.0})))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.set_volume(1.5).await.unwrap();
}

#[tokio::test]
async fn set_volume_clamps_below_0() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/volume"))
        .and(body_json(serde_json::json!({"volume": 0.0})))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.set_volume(-0.5).await.unwrap();
}
