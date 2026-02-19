mod common;

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn play_url_sends_correct_body() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/play-url"))
        .and(body_json(serde_json::json!({
            "url": "https://music.apple.com/ca/album/skin/1719860281"
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client
        .play_url("https://music.apple.com/ca/album/skin/1719860281")
        .await
        .unwrap();
}

#[tokio::test]
async fn play_item_sends_type_and_id() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/play-item"))
        .and(body_json(serde_json::json!({
            "type": "songs",
            "id": "1719861213"
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.play_item("songs", "1719861213").await.unwrap();
}

#[tokio::test]
async fn play_item_href_sends_href() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/play-item-href"))
        .and(body_json(serde_json::json!({
            "href": "/v1/catalog/ca/songs/1719861213"
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client
        .play_item_href("/v1/catalog/ca/songs/1719861213")
        .await
        .unwrap();
}

#[tokio::test]
async fn play_next_sends_type_and_id() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/play-next"))
        .and(body_json(serde_json::json!({
            "type": "songs",
            "id": "123"
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.play_next("songs", "123").await.unwrap();
}

#[tokio::test]
async fn play_later_sends_type_and_id() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/play-later"))
        .and(body_json(serde_json::json!({
            "type": "albums",
            "id": "456"
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.play_later("albums", "456").await.unwrap();
}
