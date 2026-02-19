mod common;

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn get_queue_returns_items() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/queue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::queue_json())
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;

    let queue = client.get_queue().await.unwrap();
    assert_eq!(queue.len(), 2);
    assert!(queue[0].is_current());
    assert!(!queue[1].is_current());
}

#[tokio::test]
async fn get_queue_returns_empty_on_404() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/queue"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    assert!(client.get_queue().await.unwrap().is_empty());
}

#[tokio::test]
async fn get_queue_returns_empty_on_204() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/queue"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    assert!(client.get_queue().await.unwrap().is_empty());
}

#[tokio::test]
async fn get_queue_returns_empty_on_bad_json() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/queue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"not": "an array"}"#)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    assert!(client.get_queue().await.unwrap().is_empty());
}

#[tokio::test]
async fn queue_move_to_position_sends_correct_body() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/queue/move-to-position"))
        .and(body_json(serde_json::json!({
            "startIndex": 3,
            "destinationIndex": 1
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.queue_move_to_position(3, 1).await.unwrap();
}

#[tokio::test]
async fn queue_remove_by_index_sends_correct_body() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/queue/remove-by-index"))
        .and(body_json(serde_json::json!({"index": 5})))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.queue_remove_by_index(5).await.unwrap();
}

#[tokio::test]
async fn clear_queue_ok() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/queue/clear-queue"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.clear_queue().await.unwrap();
}
