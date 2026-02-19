#![allow(dead_code)]

pub mod fixtures;

use cider_api::CiderClient;
use wiremock::MockServer;

pub async fn setup() -> (MockServer, CiderClient) {
    let server = MockServer::start().await;
    let client = CiderClient::with_base_url(server.uri());
    (server, client)
}

pub async fn setup_with_token(token: &str) -> (MockServer, CiderClient) {
    let server = MockServer::start().await;
    let client = CiderClient::with_base_url(server.uri()).with_token(token);
    (server, client)
}
