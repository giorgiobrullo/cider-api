mod common;

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn amapi_run_v3_sends_path_and_returns_json() {
    let (server, client) = common::setup().await;

    let apple_response = serde_json::json!({
        "results": { "songs": { "data": [{"id": "123"}] } }
    });

    Mock::given(method("POST"))
        .and(path("/api/v1/amapi/run-v3"))
        .and(body_json(serde_json::json!({
            "path": "/v1/catalog/us/search?term=flume"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&apple_response),
        )
        .expect(1)
        .mount(&server)
        .await;

    let result = client
        .amapi_run_v3("/v1/catalog/us/search?term=flume")
        .await
        .unwrap();
    assert!(result["results"]["songs"]["data"].is_array());
}

#[tokio::test]
async fn amapi_run_v3_error_on_500() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/amapi/run-v3"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = client
        .amapi_run_v3("/v1/me/library/songs")
        .await
        .unwrap_err();
    assert!(matches!(err, cider_api::CiderError::Http(_)));
}
