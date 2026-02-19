mod common;

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

macro_rules! test_fire_and_forget {
    ($test_name:ident, $method_name:ident, $http_method:literal, $path:literal) => {
        #[tokio::test]
        async fn $test_name() {
            let (server, client) = common::setup().await;
            Mock::given(method($http_method))
                .and(path($path))
                .respond_with(ResponseTemplate::new(200))
                .expect(1)
                .mount(&server)
                .await;
            client.$method_name().await.unwrap();
        }
    };
}

test_fire_and_forget!(play_ok, play, "POST", "/api/v1/playback/play");
test_fire_and_forget!(pause_ok, pause, "POST", "/api/v1/playback/pause");
test_fire_and_forget!(play_pause_ok, play_pause, "POST", "/api/v1/playback/playpause");
test_fire_and_forget!(stop_ok, stop, "POST", "/api/v1/playback/stop");
test_fire_and_forget!(next_ok, next, "POST", "/api/v1/playback/next");
test_fire_and_forget!(previous_ok, previous, "POST", "/api/v1/playback/previous");

// ── seek ──

#[tokio::test]
async fn seek_sends_correct_position() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/seek"))
        .and(body_json(serde_json::json!({"position": 30.0})))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.seek(30.0).await.unwrap();
}

#[tokio::test]
async fn seek_ms_converts_to_seconds() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/playback/seek"))
        .and(body_json(serde_json::json!({"position": 30.0})))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client.seek_ms(30_000).await.unwrap();
}
