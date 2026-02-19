mod common;

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn add_to_library_ok() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/add-to-library"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.add_to_library().await.unwrap();
}

#[tokio::test]
async fn set_rating_like() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/set-rating"))
        .and(body_json(serde_json::json!({"rating": 1})))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.set_rating(1).await.unwrap();
}

#[tokio::test]
async fn set_rating_dislike() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/set-rating"))
        .and(body_json(serde_json::json!({"rating": -1})))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.set_rating(-1).await.unwrap();
}

#[tokio::test]
async fn set_rating_clamps_above_1() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/set-rating"))
        .and(body_json(serde_json::json!({"rating": 1})))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.set_rating(5).await.unwrap();
}

#[tokio::test]
async fn set_rating_clamps_below_neg1() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/set-rating"))
        .and(body_json(serde_json::json!({"rating": -1})))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.set_rating(-10).await.unwrap();
}
