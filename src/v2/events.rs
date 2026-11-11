// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Real-time events from Cider (feature `events`).
//!
//! Cider pushes playback, audio, queue and playlist changes over a Socket.IO
//! endpoint. [`V2::events`](super::V2::events) connects to it and yields
//! typed [`Event`]s:
//!
//! ```no_run
//! # async fn example() -> Result<(), cider_api::CiderError> {
//! use cider_api::{v2::Event, CiderClient};
//!
//! let client = CiderClient::new();
//! let mut events = client.v2().events().await?;
//! while let Some(event) = events.next().await {
//!     match event? {
//!         Event::NowPlayingItemChanged(track) => println!("now playing {}", track.track.name),
//!         Event::VolumeChanged(volume) => println!("volume {volume}"),
//!         _ => {}
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! The connection needs no API token. Cider can deliver the same event more
//! than once, so treat events as "this is the state now", not as a count of
//! changes.

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures_util::{SinkExt, Stream, StreamExt};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::{self, Message};

use super::types::{
    AtmosState, AudioQuality, AutomixState, CaptureState, CrossfadeState, ListeningMode,
    NormalizationState, PlaybackState, QueuePosition, RadioState, RepeatMode, SmartQueueStatus,
};
use crate::client::CiderError;
use crate::types::NowPlaying;

/// Socket.IO event name Cider broadcasts on.
const CHANNEL: &str = "API:Playback";

/// How long to wait for the connection and Socket.IO handshake.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Events buffered while the consumer is not polling. Older events are kept
/// and new ones dropped once the buffer is full.
const BUFFER: usize = 1024;

// ─── Payload types ───────────────────────────────────────────────────────────

/// Payload of [`Event::NowPlayingItemChanged`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingChange {
    /// The track that is now loaded.
    #[serde(flatten)]
    pub track: NowPlaying,

    /// Audio quality code (e.g. `"256"`, `"atmos"`).
    #[serde(default)]
    pub flavor: Option<String>,

    /// Tempo from Apple's audio analysis. `None` until Cider has resolved
    /// it; a second event follows once it has.
    #[serde(default)]
    pub bpm: Option<f64>,

    /// Musical key in Camelot notation (e.g. `"8A"`). `None` until resolved.
    #[serde(default)]
    pub key: Option<String>,
}

/// Payload of [`Event::NowPlayingStatusChanged`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingStatus {
    /// `true` if the current track is in the user's library.
    #[serde(default)]
    pub in_library: bool,

    /// `true` if the current track is a favorite.
    #[serde(default)]
    pub in_favorites: bool,
}

/// A reference point for extrapolating the playback position without
/// waiting for the next event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackAnchor {
    /// Wall-clock time (Unix epoch, milliseconds) at which the track would
    /// have started if played without interruption.
    #[serde(default)]
    pub start_epoch_ms: f64,

    /// Wall-clock time (Unix epoch, milliseconds) at which the track ends.
    #[serde(default)]
    pub end_epoch_ms: f64,

    /// Position in seconds at `timestamp`.
    #[serde(default)]
    pub current_time: f64,

    /// Track duration in seconds.
    #[serde(default)]
    pub duration: f64,

    /// Wall-clock time (Unix epoch, milliseconds) the anchor was taken.
    #[serde(default)]
    pub timestamp: f64,

    /// Playback rate (`1.0` = normal speed).
    #[serde(default)]
    pub rate: f64,
}

/// Payload of [`Event::PlaybackStateChanged`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStateChange {
    /// New transport state.
    pub state: PlaybackState,

    /// The loaded track, if any.
    #[serde(default, rename = "attributes")]
    pub now_playing: Option<NowPlaying>,

    /// Position reference point, when Cider has one.
    #[serde(default)]
    pub anchor: Option<PlaybackAnchor>,
}

/// Payload of [`Event::PlaybackTimeChanged`]. Times are in seconds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackProgress {
    /// Current position.
    #[serde(default)]
    pub current_playback_time: f64,

    /// Duration of the current track.
    #[serde(default)]
    pub current_playback_duration: f64,

    /// Time left in the current track.
    #[serde(default)]
    pub current_playback_time_remaining: f64,

    /// `true` while audio is playing.
    #[serde(default)]
    pub is_playing: bool,

    /// Position reference point, when Cider has one.
    #[serde(default)]
    pub anchor: Option<PlaybackAnchor>,
}

/// How Cider moves from one track to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum TransitionMode {
    /// Tracks overlap and fade.
    Crossfade,
    /// Tracks play back to back.
    Gapless,
    /// DJ-style mix.
    Automix,
    /// A mode this crate version does not know about.
    #[serde(other)]
    Unknown,
}

/// Payload of [`Event::TransitionStarted`] and [`Event::TransitionCompleted`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    /// Kind of transition.
    pub mode: TransitionMode,

    /// ID of the track being left, if there was one.
    #[serde(default)]
    pub from: Option<String>,

    /// ID of the track being entered.
    #[serde(default)]
    pub to: Option<String>,
}

/// What a playlist write changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PlaylistOperation {
    /// The playlist was created.
    Created,
    /// Name or description changed.
    Metadata,
    /// Tracks were added, removed, replaced or reordered.
    Tracks,
    /// The playlist was restored from its history.
    Restored,
    /// An operation this crate version does not know about.
    #[serde(other)]
    Unknown,
}

/// Payload of [`Event::PlaylistChanged`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistChange {
    /// Playlist ID.
    pub id: String,

    /// What changed.
    pub operation: PlaylistOperation,

    /// Number of tracks after the change.
    #[serde(default)]
    pub track_count: u32,

    /// History restore point taken before the change, for track changes.
    #[serde(default)]
    pub snapshot_at: Option<String>,
}

#[derive(Deserialize)]
struct ModeOnly {
    mode: ListeningMode,
}

// ─── Event ───────────────────────────────────────────────────────────────────

/// An event pushed by Cider.
///
/// Events Cider adds in the future, and payloads this crate cannot parse,
/// arrive as [`Event::Other`] with the raw JSON.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Event {
    /// The current track changed (`playbackStatus.nowPlayingItemDidChange`).
    NowPlayingItemChanged(Box<NowPlayingChange>),
    /// The current track was added to or removed from the library or
    /// favorites (`playbackStatus.nowPlayingStatusDidChange`).
    NowPlayingStatusChanged(NowPlayingStatus),
    /// Playback started, paused or stopped
    /// (`playbackStatus.playbackStateDidChange`).
    PlaybackStateChanged(Box<PlaybackStateChange>),
    /// Playback position, about once per second while playing
    /// (`playbackStatus.playbackTimeDidChange`).
    PlaybackTimeChanged(PlaybackProgress),
    /// A transition between two tracks began
    /// (`playbackStatus.transitionStarted`).
    TransitionStarted(Transition),
    /// A transition between two tracks finished
    /// (`playbackStatus.transitionCompleted`).
    TransitionCompleted(Transition),
    /// Volume changed, `0.0..=1.0` (`playerStatus.volumeDidChange`).
    VolumeChanged(f32),
    /// Repeat mode changed (`playerStatus.repeatModeDidChange`).
    RepeatModeChanged(RepeatMode),
    /// Shuffle was turned on (`true`) or off
    /// (`playerStatus.shuffleModeDidChange`).
    ShuffleChanged(bool),
    /// Crossfade settings changed (`audioStatus.crossfadeChanged`).
    CrossfadeChanged(CrossfadeState),
    /// Automix settings changed (`audioStatus.automixChanged`).
    AutomixChanged(AutomixState),
    /// Dolby Atmos settings changed (`audioStatus.atmosChanged`).
    AtmosChanged(AtmosState),
    /// Listening Mode preset changed (`audioStatus.listeningModeChanged`).
    ListeningModeChanged(ListeningMode),
    /// Radio Sound settings changed (`audioStatus.radioModeChanged`).
    RadioModeChanged(RadioState),
    /// Host audio capture settings changed (`audioStatus.captureChanged`).
    /// `supported` is not sent with the event and is always `false` here.
    CaptureChanged(CaptureState),
    /// Volume normalization settings changed
    /// (`audioStatus.normalizationChanged`).
    NormalizationChanged(NormalizationState),
    /// Audio quality of the current track changed
    /// (`audioStatus.qualityChanged`).
    QualityChanged(AudioQuality),
    /// The queue was modified or the position in it moved
    /// (`queueStatus.queueChanged`).
    QueueChanged(QueuePosition),
    /// Smart Queue Optimizer status changed
    /// (`queueStatus.smartQueueChanged`).
    SmartQueueChanged(SmartQueueStatus),
    /// A playlist was written, from the API or from Cider's UI
    /// (`libraryStatus.playlistChanged`).
    PlaylistChanged(PlaylistChange),
    /// An event this crate version has no type for.
    Other {
        /// Cider's event name, e.g. `"playbackStatus.somethingNew"`.
        kind: String,
        /// Raw payload.
        data: Value,
    },
}

fn parse<T: DeserializeOwned>(data: &Value) -> Option<T> {
    T::deserialize(data).ok()
}

/// Cider sends on/off values as `0`/`1`, occasionally as booleans.
fn flag(data: &Value) -> Option<bool> {
    data.as_bool().or_else(|| data.as_u64().map(|n| n != 0))
}

impl Event {
    /// Build an event from Cider's `{ type, data }` message.
    ///
    /// Useful when the messages come from somewhere other than
    /// [`EventStream`], e.g. your own Socket.IO client.
    #[must_use]
    pub fn from_message(kind: &str, data: Value) -> Self {
        let parsed = match kind {
            "playbackStatus.nowPlayingItemDidChange" => {
                parse(&data).map(|c| Self::NowPlayingItemChanged(Box::new(c)))
            }
            "playbackStatus.nowPlayingStatusDidChange" => {
                parse(&data).map(Self::NowPlayingStatusChanged)
            }
            "playbackStatus.playbackStateDidChange" => {
                parse(&data).map(|c| Self::PlaybackStateChanged(Box::new(c)))
            }
            "playbackStatus.playbackTimeDidChange" => parse(&data).map(Self::PlaybackTimeChanged),
            "playbackStatus.transitionStarted" => parse(&data).map(Self::TransitionStarted),
            "playbackStatus.transitionCompleted" => parse(&data).map(Self::TransitionCompleted),
            #[allow(clippy::cast_possible_truncation)]
            "playerStatus.volumeDidChange" => data.as_f64().map(|v| Self::VolumeChanged(v as f32)),
            "playerStatus.repeatModeDidChange" => match data.as_u64() {
                Some(0) => Some(Self::RepeatModeChanged(RepeatMode::Off)),
                Some(1) => Some(Self::RepeatModeChanged(RepeatMode::One)),
                Some(2) => Some(Self::RepeatModeChanged(RepeatMode::All)),
                _ => parse(&data).map(Self::RepeatModeChanged),
            },
            "playerStatus.shuffleModeDidChange" => flag(&data).map(Self::ShuffleChanged),
            "audioStatus.crossfadeChanged" => parse(&data).map(Self::CrossfadeChanged),
            "audioStatus.automixChanged" => parse(&data).map(Self::AutomixChanged),
            "audioStatus.atmosChanged" => parse(&data).map(Self::AtmosChanged),
            "audioStatus.listeningModeChanged" => {
                parse::<ModeOnly>(&data).map(|m| Self::ListeningModeChanged(m.mode))
            }
            "audioStatus.radioModeChanged" => parse(&data).map(Self::RadioModeChanged),
            "audioStatus.captureChanged" => parse(&data).map(Self::CaptureChanged),
            "audioStatus.normalizationChanged" => parse(&data).map(Self::NormalizationChanged),
            "audioStatus.qualityChanged" => parse(&data).map(Self::QualityChanged),
            "queueStatus.queueChanged" => parse(&data).map(Self::QueueChanged),
            "queueStatus.smartQueueChanged" => parse(&data).map(Self::SmartQueueChanged),
            "libraryStatus.playlistChanged" => parse(&data).map(Self::PlaylistChanged),
            _ => None,
        };
        parsed.unwrap_or_else(|| Self::Other {
            kind: kind.to_owned(),
            data,
        })
    }
}

// ─── Stream ──────────────────────────────────────────────────────────────────

/// A live connection to Cider's event channel. Created by
/// [`V2::events`](super::V2::events).
///
/// Call [`next`](Self::next) in a loop, or use it as a
/// [`Stream`](futures_util::Stream). The stream ends (`None`) when Cider
/// closes the connection, e.g. on quit; a connection failure is yielded once
/// as `Some(Err(_))` first. Reconnect by calling `events()` again.
///
/// The connection is serviced by a background task, so it stays alive even
/// if you poll rarely. Dropping the stream closes it.
#[derive(Debug)]
pub struct EventStream {
    rx: mpsc::Receiver<Result<Event, CiderError>>,
    task: JoinHandle<()>,
}

impl EventStream {
    /// Wait for the next event.
    pub async fn next(&mut self) -> Option<Result<Event, CiderError>> {
        self.rx.recv().await
    }
}

impl Stream for EventStream {
    type Item = Result<Event, CiderError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

impl Drop for EventStream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenPacket {
    #[serde(default = "default_ping_interval")]
    ping_interval: u64,
    #[serde(default = "default_ping_timeout")]
    ping_timeout: u64,
}

fn default_ping_interval() -> u64 {
    25_000
}

fn default_ping_timeout() -> u64 {
    20_000
}

#[derive(Deserialize)]
struct WireEvent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    data: Value,
}

fn stream_err(context: &str, e: impl std::fmt::Display) -> CiderError {
    CiderError::Events(format!("{context}: {e}"))
}

fn connect_err(e: tungstenite::Error) -> CiderError {
    match e {
        tungstenite::Error::Io(_) => CiderError::NotReachable,
        tungstenite::Error::Http(resp) if resp.status().as_u16() == 404 => CiderError::Unsupported,
        other => stream_err("connection failed", other),
    }
}

/// Turn `http://host:port` into the Socket.IO websocket URL.
fn socket_url(base_url: &str) -> Result<String, CiderError> {
    let rest = base_url
        .strip_prefix("http://")
        .ok_or_else(|| CiderError::Events("events need an http:// base URL".into()))?;
    Ok(format!(
        "ws://{}/socket.io/?EIO=4&transport=websocket",
        rest.trim_end_matches('/')
    ))
}

/// Connect, complete the Engine.IO and Socket.IO handshakes, and start the
/// background task that feeds the stream.
pub(crate) async fn connect(base_url: &str) -> Result<EventStream, CiderError> {
    let url = socket_url(base_url)?;
    let handshake = async {
        let (mut ws, _) = tokio_tungstenite::connect_async(url.as_str())
            .await
            .map_err(connect_err)?;

        // Engine.IO open packet: `0{"sid":…,"pingInterval":…,"pingTimeout":…}`.
        let open = next_text(&mut ws).await?;
        let open: OpenPacket = open
            .strip_prefix('0')
            .and_then(|json| serde_json::from_str(json).ok())
            .ok_or_else(|| CiderError::Events("unexpected handshake from Cider".into()))?;

        // Socket.IO connect to the default namespace, answered with `40{…}`.
        ws.send(Message::text("40"))
            .await
            .map_err(|e| stream_err("handshake failed", e))?;
        loop {
            let packet = next_text(&mut ws).await?;
            if packet.starts_with("40") {
                break;
            }
            if let Some(reason) = packet.strip_prefix("44") {
                return Err(CiderError::Events(format!(
                    "Cider refused the connection: {reason}"
                )));
            }
            if packet == "2" {
                ws.send(Message::text("3"))
                    .await
                    .map_err(|e| stream_err("handshake failed", e))?;
            }
        }
        Ok((ws, open))
    };

    let (ws, open) = tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake)
        .await
        .map_err(|_| CiderError::NotReachable)??;

    // The server pings every `ping_interval`; silence for longer than that
    // plus `ping_timeout` means the connection is dead.
    let silence = Duration::from_millis(open.ping_interval.saturating_add(open.ping_timeout));
    let (tx, rx) = mpsc::channel(BUFFER);
    let task = tokio::spawn(pump(ws, tx, silence));
    Ok(EventStream { rx, task })
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn next_text(ws: &mut Socket) -> Result<String, CiderError> {
    loop {
        match ws.next().await {
            Some(Ok(Message::Text(text))) => return Ok(text.as_str().to_owned()),
            Some(Ok(Message::Close(_))) | None => {
                return Err(CiderError::Events(
                    "Cider closed the connection during the handshake".into(),
                ))
            }
            Some(Ok(_)) => {}
            Some(Err(e)) => return Err(stream_err("handshake failed", e)),
        }
    }
}

async fn pump(mut ws: Socket, tx: mpsc::Sender<Result<Event, CiderError>>, silence: Duration) {
    loop {
        let message = match tokio::time::timeout(silence, ws.next()).await {
            Err(_) => {
                let _ = tx
                    .send(Err(CiderError::Events(
                        "no heartbeat from Cider; connection lost".into(),
                    )))
                    .await;
                return;
            }
            Ok(None | Some(Ok(Message::Close(_)))) => return,
            Ok(Some(Err(e))) => {
                let _ = tx.send(Err(stream_err("connection lost", e))).await;
                return;
            }
            Ok(Some(Ok(message))) => message,
        };
        let Message::Text(text) = message else {
            continue;
        };
        let text = text.as_str();

        if text == "2" {
            // Engine.IO ping → pong.
            if let Err(e) = ws.send(Message::text("3")).await {
                let _ = tx.send(Err(stream_err("connection lost", e))).await;
                return;
            }
        } else if text == "41" || text == "1" {
            // Socket.IO disconnect / Engine.IO close.
            return;
        } else if let Some(payload) = text.strip_prefix("42") {
            let Ok((name, body)) = serde_json::from_str::<(String, Value)>(payload) else {
                continue;
            };
            if name != CHANNEL {
                continue;
            }
            let Ok(wire) = WireEvent::deserialize(body) else {
                continue;
            };
            let event = Event::from_message(&wire.kind, wire.data);
            match tx.try_send(Ok(event)) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => {
                    tracing::warn!("event buffer full; dropping a Cider event");
                }
                Err(mpsc::error::TrySendError::Closed(_)) => return,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn socket_url_from_base_url() {
        assert_eq!(
            socket_url("http://127.0.0.1:10767").unwrap(),
            "ws://127.0.0.1:10767/socket.io/?EIO=4&transport=websocket"
        );
        assert!(socket_url("https://example.com").is_err());
    }

    #[test]
    fn scalar_events() {
        assert!(matches!(
            Event::from_message("playerStatus.volumeDidChange", json!(0.27)),
            Event::VolumeChanged(v) if (v - 0.27).abs() < 1e-6
        ));
        assert!(matches!(
            Event::from_message("playerStatus.repeatModeDidChange", json!(2)),
            Event::RepeatModeChanged(RepeatMode::All)
        ));
        assert!(matches!(
            Event::from_message("playerStatus.shuffleModeDidChange", json!(1)),
            Event::ShuffleChanged(true)
        ));
        assert!(matches!(
            Event::from_message("playerStatus.shuffleModeDidChange", json!(0)),
            Event::ShuffleChanged(false)
        ));
    }

    #[test]
    fn unknown_and_malformed_events_fall_back_to_other() {
        let event = Event::from_message("playbackStatus.somethingNew", json!({"a": 1}));
        assert!(matches!(
            &event,
            Event::Other { kind, data } if kind == "playbackStatus.somethingNew" && data["a"] == 1
        ));
        // Known name, payload of the wrong shape.
        let event = Event::from_message("playerStatus.volumeDidChange", json!("loud"));
        assert!(matches!(event, Event::Other { .. }));
    }

    #[test]
    fn now_playing_item_as_sent_by_cider_4() {
        let event = Event::from_message(
            "playbackStatus.nowPlayingItemDidChange",
            json!({
                "albumName": "Negative Spaces", "artistName": "Poppy", "name": "tomorrow",
                "artwork": {"height": 1400, "width": 1400, "url": "https://example.com/600x600bb.png"},
                "durationInMillis": 51611, "playParams": {"id": "1765105160", "kind": "song"},
                "currentPlaybackTime": 32.03, "remainingTime": 189.27, "inLibrary": false,
                "inFavorites": false, "shuffleMode": 0, "repeatMode": 0,
                "flavor": "256", "bpm": 149, "key": "2B"
            }),
        );
        let Event::NowPlayingItemChanged(change) = event else {
            panic!("wrong variant: {event:?}");
        };
        assert_eq!(change.track.name, "tomorrow");
        assert_eq!(change.track.duration_in_millis, 51611);
        assert_eq!(change.flavor.as_deref(), Some("256"));
        assert_eq!(change.bpm, Some(149.0));
        assert_eq!(change.key.as_deref(), Some("2B"));
    }

    #[test]
    fn playback_state_and_time_as_sent_by_cider_4() {
        let event = Event::from_message(
            "playbackStatus.playbackStateDidChange",
            json!({"attributes": {"name": "halo"}, "state": "playing", "anchor": null}),
        );
        let Event::PlaybackStateChanged(change) = event else {
            panic!("wrong variant: {event:?}");
        };
        assert_eq!(change.state, PlaybackState::Playing);
        assert_eq!(change.now_playing.unwrap().name, "halo");
        assert!(change.anchor.is_none());

        let event = Event::from_message(
            "playbackStatus.playbackTimeDidChange",
            json!({
                "currentPlaybackTime": 2.005685, "currentPlaybackDuration": 221.316,
                "currentPlaybackTimeRemaining": 219.310315, "isPlaying": true,
                "anchor": {"startEpochMs": 1794405027002u64, "endEpochMs": 1794405248318u64,
                           "currentTime": 2.005685, "duration": 221.316,
                           "timestamp": 1794405029008u64, "rate": 1}
            }),
        );
        let Event::PlaybackTimeChanged(progress) = event else {
            panic!("wrong variant: {event:?}");
        };
        assert!(progress.is_playing);
        assert!((progress.current_playback_duration - 221.316).abs() < 1e-9);
        assert!((progress.anchor.unwrap().rate - 1.0).abs() < 1e-9);
    }

    #[test]
    fn queue_playlist_and_audio_events() {
        assert!(matches!(
            Event::from_message(
                "queueStatus.queueChanged",
                json!({"position": 13, "total": 15})
            ),
            Event::QueueChanged(QueuePosition {
                position: 13,
                total: 15
            })
        ));
        let event = Event::from_message(
            "libraryStatus.playlistChanged",
            json!({"id": "p.abc", "operation": "tracks", "trackCount": 3, "snapshotAt": null}),
        );
        assert!(matches!(
            event,
            Event::PlaylistChanged(PlaylistChange {
                operation: PlaylistOperation::Tracks,
                track_count: 3,
                ..
            })
        ));
        assert!(matches!(
            Event::from_message("audioStatus.listeningModeChanged", json!({"mode": "cozy"})),
            Event::ListeningModeChanged(ListeningMode::Cozy)
        ));
        assert!(matches!(
            Event::from_message(
                "audioStatus.normalizationChanged",
                json!({"enabled": true, "targetLufs": -14})
            ),
            Event::NormalizationChanged(_)
        ));
        assert!(matches!(
            Event::from_message(
                "playbackStatus.transitionStarted",
                json!({"mode": "crossfade", "from": null, "to": "123"})
            ),
            Event::TransitionStarted(Transition {
                mode: TransitionMode::Crossfade,
                from: None,
                ..
            })
        ));
    }
}
