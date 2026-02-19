mod common;

use wiremock::matchers::{method, path};
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

test_fire_and_forget!(toggle_repeat_ok, toggle_repeat, "POST", "/api/v1/playback/toggle-repeat");
test_fire_and_forget!(toggle_shuffle_ok, toggle_shuffle, "POST", "/api/v1/playback/toggle-shuffle");
test_fire_and_forget!(toggle_autoplay_ok, toggle_autoplay, "POST", "/api/v1/playback/toggle-autoplay");

#[tokio::test]
async fn get_repeat_mode_returns_value() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/repeat-mode"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::repeat_mode_json(2))
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    assert_eq!(client.get_repeat_mode().await.unwrap(), 2);
}

#[tokio::test]
async fn get_shuffle_mode_returns_value() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/shuffle-mode"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::shuffle_mode_json(1))
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    assert_eq!(client.get_shuffle_mode().await.unwrap(), 1);
}

#[tokio::test]
async fn get_autoplay_returns_true() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/autoplay"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::autoplay_json(true))
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    assert!(client.get_autoplay().await.unwrap());
}

#[tokio::test]
async fn get_autoplay_returns_false() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/playback/autoplay"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::autoplay_json(false))
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    assert!(!client.get_autoplay().await.unwrap());
}
