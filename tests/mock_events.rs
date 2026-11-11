//! Event stream tests against a minimal in-process Socket.IO server.
#![cfg(feature = "events")]

use cider_api::v2::{Event, RepeatMode};
use cider_api::{CiderClient, CiderError};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// What the fake server does after the Socket.IO handshake.
enum Script {
    /// Send these packets, then close.
    Send(Vec<&'static str>),
    /// Send a ping and report whether a pong came back, then close.
    Ping(tokio::sync::oneshot::Sender<bool>),
}

async fn serve(script: Script) -> CiderClient {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
        ws.send(Message::text(
            r#"0{"sid":"abc","upgrades":[],"pingInterval":25000,"pingTimeout":20000,"maxPayload":1000000}"#,
        ))
        .await
        .unwrap();
        let connect = ws.next().await.unwrap().unwrap();
        assert_eq!(connect.to_text().unwrap(), "40");
        ws.send(Message::text(r#"40{"sid":"def"}"#)).await.unwrap();
        ws.send(Message::text(r#"42["message","Hello from server"]"#))
            .await
            .unwrap();

        match script {
            Script::Send(packets) => {
                for packet in packets {
                    ws.send(Message::text(packet)).await.unwrap();
                }
            }
            Script::Ping(result) => {
                ws.send(Message::text("2")).await.unwrap();
                let pong = ws.next().await.and_then(Result::ok);
                let _ = result.send(pong.is_some_and(|m| m.to_text().ok() == Some("3")));
            }
        }
        let _ = ws.close(None).await;
    });
    CiderClient::with_base_url(format!("http://{addr}"))
}

#[tokio::test]
async fn yields_typed_events_then_ends_when_cider_closes() {
    let client = serve(Script::Send(vec![
        r#"42["API:Playback",{"type":"playerStatus.volumeDidChange","data":0.27}]"#,
        r#"42["API:Playback",{"type":"playerStatus.repeatModeDidChange","data":2}]"#,
        // Other Socket.IO events and malformed packets are ignored.
        r#"42["Something:Else",{"type":"x","data":1}]"#,
        r#"42[not json"#,
        r#"42["API:Playback",{"type":"queueStatus.queueChanged","data":{"position":3,"total":15}}]"#,
        r#"42["API:Playback",{"type":"future.event","data":{"a":1}}]"#,
    ]))
    .await;

    let mut events = client.v2().events().await.unwrap();

    let first = events.next().await.unwrap().unwrap();
    assert!(matches!(first, Event::VolumeChanged(v) if (v - 0.27).abs() < 1e-6));
    let second = events.next().await.unwrap().unwrap();
    assert!(matches!(second, Event::RepeatModeChanged(RepeatMode::All)));
    let third = events.next().await.unwrap().unwrap();
    let Event::QueueChanged(queue) = third else {
        panic!("wrong variant: {third:?}");
    };
    assert_eq!((queue.position, queue.total), (3, 15));
    let fourth = events.next().await.unwrap().unwrap();
    assert!(matches!(fourth, Event::Other { ref kind, .. } if kind == "future.event"));

    assert!(events.next().await.is_none());
}

#[tokio::test]
async fn works_as_a_futures_stream() {
    let client = serve(Script::Send(vec![
        r#"42["API:Playback",{"type":"playerStatus.shuffleModeDidChange","data":1}]"#,
    ]))
    .await;

    let events = client.v2().events().await.unwrap();
    let all: Vec<_> = StreamExt::collect(events).await;
    assert_eq!(all.len(), 1);
    assert!(matches!(all[0], Ok(Event::ShuffleChanged(true))));
}

#[tokio::test]
async fn answers_pings_without_being_polled() {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let client = serve(Script::Ping(tx)).await;

    // Hold the stream but never poll it: the background task must still pong.
    let _events = client.v2().events().await.unwrap();
    assert!(rx.await.unwrap(), "server did not receive a pong");
}

#[tokio::test]
async fn not_reachable_when_nothing_listens() {
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let e = CiderClient::with_port(port)
        .v2()
        .events()
        .await
        .unwrap_err();
    assert!(matches!(e, CiderError::NotReachable), "got {e:?}");
}
