// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Async HTTP client for the Cider REST API.

use std::time::Duration;

use reqwest::Client;
use thiserror::Error;
use tracing::{debug, instrument, warn};

use crate::types::{
    AmApiRequest, ApiResponse, AutoplayResponse, IsPlayingResponse, NowPlaying,
    NowPlayingResponse, PlayItemHrefRequest, PlayItemRequest, PlayUrlRequest, QueueItem,
    QueueMoveRequest, QueueRemoveRequest, RatingRequest, RepeatModeResponse, SeekRequest,
    ShuffleModeResponse, VolumeRequest, VolumeResponse,
};

/// Default Cider RPC port.
pub const DEFAULT_PORT: u16 = 10767;

/// Connection timeout — short because the server is localhost.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(1);

/// Per-request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Errors returned by [`CiderClient`] methods.
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// use cider_api::{CiderClient, CiderError};
///
/// let client = CiderClient::new();
/// match client.is_active().await {
///     Ok(()) => println!("Cider is running"),
///     Err(CiderError::Unauthorized) => println!("Bad API token"),
///     Err(CiderError::Http(e)) if e.is_connect() => println!("Cider not running"),
///     Err(e) => println!("Error: {e}"),
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Error)]
pub enum CiderError {
    /// An HTTP-level error from [`reqwest`].
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Cider is not running or the port is unreachable.
    #[error("Cider is not running or not reachable")]
    NotReachable,

    /// The API token was rejected (HTTP 401/403).
    #[error("Invalid API token")]
    Unauthorized,

    /// No track is currently loaded.
    #[error("No track currently playing")]
    NothingPlaying,

    /// Catch-all for unexpected API responses.
    #[error("API error: {0}")]
    Api(String),
}

/// Async client for the [Cider](https://cider.sh) music player REST API.
///
/// Communicates with Cider's local HTTP server (default `http://127.0.0.1:10767`)
/// to control playback, manage the queue, and query track information.
///
/// # Construction
///
/// ```
/// use cider_api::CiderClient;
///
/// // Default (localhost:10767, no auth)
/// let client = CiderClient::new();
///
/// // Custom port
/// let client = CiderClient::with_port(9999);
///
/// // With authentication
/// let client = CiderClient::new().with_token("my-token");
/// ```
///
/// The client is cheaply [`Clone`]able — it shares an inner connection pool.
///
/// # Errors
///
/// All async methods return `Result<_, CiderError>`. Common error cases:
///
/// - [`CiderError::Http`] — network or connection failure.
/// - [`CiderError::Unauthorized`] — invalid API token (HTTP 401/403).
/// - [`CiderError::Api`] — unexpected response from Cider.
#[derive(Debug, Clone)]
pub struct CiderClient {
    http: Client,
    base_url: String,
    api_token: Option<String>,
}

impl CiderClient {
    /// Create a new client targeting `http://127.0.0.1:10767`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_port(DEFAULT_PORT)
    }

    /// Create a new client targeting `http://127.0.0.1:{port}`.
    ///
    /// # Panics
    ///
    /// Panics if the underlying HTTP client cannot be constructed (only
    /// possible if TLS initialisation fails at the OS level).
    #[must_use]
    pub fn with_port(port: u16) -> Self {
        let http = Client::builder()
            .connect_timeout(CONNECTION_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .pool_max_idle_per_host(2)
            .pool_idle_timeout(Duration::from_secs(10))
            .tcp_keepalive(None)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            base_url: format!("http://127.0.0.1:{port}"),
            api_token: None,
        }
    }

    /// Create a client targeting an arbitrary base URL.
    ///
    /// This is intended for testing (e.g. pointing at a mock server).
    #[doc(hidden)]
    #[must_use]
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let http = Client::builder()
            .connect_timeout(CONNECTION_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .pool_max_idle_per_host(2)
            .pool_idle_timeout(Duration::from_secs(10))
            .tcp_keepalive(None)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            base_url: base_url.into(),
            api_token: None,
        }
    }

    /// Attach an API token for authentication.
    ///
    /// The token is sent in the `apptoken` header on every request.
    /// Generate one in Cider under **Settings > Connectivity > Manage External
    /// Application Access**.
    #[must_use]
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.api_token = Some(token.into());
        self
    }

    // ── Internal helpers ─────────────────────────────────────────────────

    /// Build a request under `/api/v1/playback`.
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}/api/v1/playback{}", self.base_url, path);
        let mut req = self.http.request(method, &url);
        if let Some(token) = &self.api_token {
            req = req.header("apptoken", token);
        }
        req
    }

    /// Build a request under an arbitrary API path.
    fn request_raw(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.http.request(method, &url);
        if let Some(token) = &self.api_token {
            req = req.header("apptoken", token);
        }
        req
    }

    // ── Status ───────────────────────────────────────────────────────────

    /// Check that Cider is running and the RPC server is reachable.
    ///
    /// Sends `GET /active` — Cider responds with `204 No Content` if alive.
    ///
    /// # Errors
    ///
    /// - [`CiderError::Unauthorized`] if the token is wrong.
    /// - [`CiderError::Api`] if the connection is refused or times out.
    #[instrument(skip(self), fields(base_url = %self.base_url))]
    pub async fn is_active(&self) -> Result<(), CiderError> {
        debug!("Checking Cider connection");

        let resp = self
            .request(reqwest::Method::GET, "/active")
            .send()
            .await
            .map_err(|e| {
                warn!("Connection error: {e:?}");
                if e.is_connect() {
                    CiderError::Api(format!("Connection refused ({e})"))
                } else if e.is_timeout() {
                    CiderError::Api("Connection timed out".to_string())
                } else {
                    CiderError::Api(format!("Network error ({e})"))
                }
            })?;

        debug!("Response status: {}", resp.status());

        match resp.status().as_u16() {
            200 | 204 => Ok(()),
            401 | 403 => Err(CiderError::Unauthorized),
            _ => Err(CiderError::Api(format!(
                "Unexpected response (HTTP {})",
                resp.status().as_u16()
            ))),
        }
    }

    /// Check whether music is currently playing.
    ///
    /// Sends `GET /is-playing`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the response cannot be parsed.
    pub async fn is_playing(&self) -> Result<bool, CiderError> {
        let resp: ApiResponse<IsPlayingResponse> = self
            .request(reqwest::Method::GET, "/is-playing")
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data.is_playing)
    }

    /// Get the currently playing track.
    ///
    /// Returns `None` if nothing is loaded. The returned [`NowPlaying`] includes
    /// both Apple Music catalog metadata and live playback state
    /// (`current_playback_time`, `remaining_time`, etc.).
    ///
    /// Sends `GET /now-playing`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] on network failure. Returns `Ok(None)` (not an
    /// error) if nothing is playing or the response cannot be parsed.
    pub async fn now_playing(&self) -> Result<Option<NowPlaying>, CiderError> {
        let resp = self
            .request(reqwest::Method::GET, "/now-playing")
            .send()
            .await?;

        if resp.status() == 404 || resp.status() == 204 {
            return Ok(None);
        }

        match resp.json::<ApiResponse<NowPlayingResponse>>().await {
            Ok(data) => Ok(Some(data.data.info)),
            Err(_) => Ok(None),
        }
    }

    // ── Playback control ─────────────────────────────────────────────────

    /// Resume playback.
    ///
    /// If nothing is loaded, the behaviour set under
    /// **Settings > Play Button on Stopped Action** takes effect.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn play(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/play")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Pause the current track. No-op if already paused or nothing is playing.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn pause(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/pause")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Toggle between playing and paused.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn play_pause(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/playpause")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Stop playback and unload the current track. Queue items are kept.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn stop(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/stop")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Skip to the next track in the queue.
    ///
    /// Respects autoplay status if the queue is empty.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn next(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/next")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Go back to the previously played track (from playback history).
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn previous(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/previous")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Seek to a position in the current track.
    ///
    /// # Arguments
    ///
    /// * `position_secs` — target offset in **seconds** (e.g. `30.0`).
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn seek(&self, position_secs: f64) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/seek")
            .json(&SeekRequest {
                position: position_secs,
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Convenience wrapper for [`seek`](Self::seek) that accepts milliseconds.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn seek_ms(&self, position_ms: u64) -> Result<(), CiderError> {
        #[allow(clippy::cast_precision_loss)] // ms precision loss only above ~143 million years
        let secs = position_ms as f64 / 1000.0;
        self.seek(secs).await
    }

    // ── Play items ───────────────────────────────────────────────────────

    /// Start playback of an Apple Music URL.
    ///
    /// The URL can be obtained from **Share > Apple Music** in Cider or the
    /// Apple Music web player.
    ///
    /// # Arguments
    ///
    /// * `url` — e.g. `"https://music.apple.com/ca/album/…/1719860281"`
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn play_url(&self, url: &str) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/play-url")
            .json(&PlayUrlRequest {
                url: url.to_string(),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Start playback of an item by Apple Music type and catalog ID.
    ///
    /// # Arguments
    ///
    /// * `item_type` — Apple Music type: `"songs"`, `"albums"`, `"playlists"`, etc.
    /// * `id` — catalog ID as a **string** (e.g. `"1719861213"`).
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn play_item(&self, item_type: &str, id: &str) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/play-item")
            .json(&PlayItemRequest {
                item_type: item_type.to_string(),
                id: id.to_string(),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Start playback of an item by its Apple Music API href.
    ///
    /// # Arguments
    ///
    /// * `href` — API path, e.g. `"/v1/catalog/ca/songs/1719861213"`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn play_item_href(&self, href: &str) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/play-item-href")
            .json(&PlayItemHrefRequest {
                href: href.to_string(),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Add an item to the **start** of the queue (plays next).
    ///
    /// # Arguments
    ///
    /// * `item_type` — `"songs"`, `"albums"`, etc.
    /// * `id` — catalog ID as a string.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn play_next(&self, item_type: &str, id: &str) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/play-next")
            .json(&PlayItemRequest {
                item_type: item_type.to_string(),
                id: id.to_string(),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Add an item to the **end** of the queue (plays last).
    ///
    /// # Arguments
    ///
    /// * `item_type` — `"songs"`, `"albums"`, etc.
    /// * `id` — catalog ID as a string.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn play_later(&self, item_type: &str, id: &str) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/play-later")
            .json(&PlayItemRequest {
                item_type: item_type.to_string(),
                id: id.to_string(),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    // ── Queue ────────────────────────────────────────────────────────────

    /// Get the current playback queue.
    ///
    /// Returns a [`Vec<QueueItem>`] that includes history items, the currently
    /// playing track, and upcoming items. Use [`QueueItem::is_current`] to
    /// find the active track.
    ///
    /// Returns an empty `Vec` if the queue is empty or the response format is
    /// unexpected.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] on network failure. Returns `Ok(vec![])` (not an
    /// error) if the queue is empty or the format is unrecognised.
    pub async fn get_queue(&self) -> Result<Vec<QueueItem>, CiderError> {
        let resp = self
            .request(reqwest::Method::GET, "/queue")
            .send()
            .await?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::NO_CONTENT {
            return Ok(vec![]);
        }

        let text = resp.text().await?;
        match serde_json::from_str::<Vec<QueueItem>>(&text) {
            Ok(items) => Ok(items),
            Err(_) => Ok(vec![]),
        }
    }

    /// Move a queue item from one position to another.
    ///
    /// Both indices are **1-based**. The queue includes history items, so the
    /// first visible "Up Next" item may not be at index 1.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn queue_move_to_position(
        &self,
        start_index: u32,
        destination_index: u32,
    ) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/queue/move-to-position")
            .json(&QueueMoveRequest {
                start_index,
                destination_index,
                return_queue: None,
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Remove a queue item by its **1-based** index.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn queue_remove_by_index(&self, index: u32) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/queue/remove-by-index")
            .json(&QueueRemoveRequest { index })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Clear all items from the queue.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn clear_queue(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/queue/clear-queue")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    // ── Volume ───────────────────────────────────────────────────────────

    /// Get the current volume (`0.0` = muted, `1.0` = full).
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the response cannot be parsed.
    pub async fn get_volume(&self) -> Result<f32, CiderError> {
        let resp: ApiResponse<VolumeResponse> = self
            .request(reqwest::Method::GET, "/volume")
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data.volume)
    }

    /// Set the volume. Values are clamped to `0.0..=1.0`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn set_volume(&self, volume: f32) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/volume")
            .json(&VolumeRequest {
                volume: volume.clamp(0.0, 1.0),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    // ── Library / ratings ────────────────────────────────────────────────

    /// Add the currently playing track to the user's library.
    ///
    /// No-op if the track is already in the library.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn add_to_library(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/add-to-library")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Rate the currently playing track.
    ///
    /// * `-1` — dislike
    /// * `0` — remove rating
    /// * `1` — like
    ///
    /// The value is clamped to `-1..=1`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn set_rating(&self, rating: i8) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/set-rating")
            .json(&RatingRequest {
                rating: rating.clamp(-1, 1),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    // ── Repeat / shuffle / autoplay ──────────────────────────────────────

    /// Get the current repeat mode.
    ///
    /// * `0` — off
    /// * `1` — repeat this song
    /// * `2` — repeat all
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the response cannot be parsed.
    pub async fn get_repeat_mode(&self) -> Result<u8, CiderError> {
        let resp: ApiResponse<RepeatModeResponse> = self
            .request(reqwest::Method::GET, "/repeat-mode")
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data.value)
    }

    /// Cycle repeat mode: **repeat one > repeat all > off**.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn toggle_repeat(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/toggle-repeat")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Get the current shuffle mode (`0` = off, `1` = on).
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the response cannot be parsed.
    pub async fn get_shuffle_mode(&self) -> Result<u8, CiderError> {
        let resp: ApiResponse<ShuffleModeResponse> = self
            .request(reqwest::Method::GET, "/shuffle-mode")
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data.value)
    }

    /// Toggle shuffle on/off.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn toggle_shuffle(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/toggle-shuffle")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Get the current autoplay status (`true` = on).
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the response cannot be parsed.
    pub async fn get_autoplay(&self) -> Result<bool, CiderError> {
        let resp: ApiResponse<AutoplayResponse> = self
            .request(reqwest::Method::GET, "/autoplay")
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data.value)
    }

    /// Toggle autoplay on/off.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn toggle_autoplay(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/toggle-autoplay")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    // ── Apple Music API passthrough ──────────────────────────────────────

    /// Execute a raw Apple Music API request via Cider's passthrough.
    ///
    /// Sends `POST /api/v1/amapi/run-v3` with the given `path`, and returns
    /// the raw JSON response from Apple Music.
    ///
    /// # Arguments
    ///
    /// * `path` — Apple Music API path, e.g. `"/v1/me/library/songs"` or
    ///   `"/v1/catalog/us/search?term=flume&types=songs"`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the response cannot be parsed.
    pub async fn amapi_run_v3(&self, path: &str) -> Result<serde_json::Value, CiderError> {
        let resp = self
            .request_raw(reqwest::Method::POST, "/api/v1/amapi/run-v3")
            .json(&AmApiRequest {
                path: path.to_string(),
            })
            .send()
            .await?
            .error_for_status()?;

        resp.json().await.map_err(CiderError::from)
    }
}

impl Default for CiderClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_client() {
        let client = CiderClient::new();
        assert_eq!(client.base_url, "http://127.0.0.1:10767");
        assert!(client.api_token.is_none());
    }

    #[test]
    fn client_with_token() {
        let client = CiderClient::new().with_token("test-token");
        assert_eq!(client.api_token, Some("test-token".to_string()));
    }

    #[test]
    fn client_custom_port() {
        let client = CiderClient::with_port(9999);
        assert_eq!(client.base_url, "http://127.0.0.1:9999");
    }

    #[test]
    fn client_is_clone() {
        let a = CiderClient::new();
        let b = a.clone();
        assert_eq!(a.base_url, b.base_url);
    }

    #[test]
    fn default_trait_same_as_new() {
        let a = CiderClient::new();
        let b = CiderClient::default();
        assert_eq!(a.base_url, b.base_url);
        assert_eq!(a.api_token, b.api_token);
    }

    #[test]
    fn with_base_url_sets_arbitrary_url() {
        let client = CiderClient::with_base_url("http://example.com:1234");
        assert_eq!(client.base_url, "http://example.com:1234");
        assert!(client.api_token.is_none());
    }

    #[test]
    fn with_token_is_chainable() {
        let client = CiderClient::with_port(8080).with_token("tok");
        assert_eq!(client.base_url, "http://127.0.0.1:8080");
        assert_eq!(client.api_token, Some("tok".to_string()));
    }

    #[test]
    fn with_token_accepts_owned_string() {
        let token = String::from("owned-token");
        let client = CiderClient::new().with_token(token);
        assert_eq!(client.api_token, Some("owned-token".to_string()));
    }

    #[test]
    fn request_builds_correct_url() {
        let client = CiderClient::with_port(9999);
        let req = client.request(reqwest::Method::GET, "/active");
        let built = req.build().unwrap();
        assert_eq!(
            built.url().as_str(),
            "http://127.0.0.1:9999/api/v1/playback/active"
        );
    }

    #[test]
    fn request_raw_builds_correct_url() {
        let client = CiderClient::with_port(9999);
        let req = client.request_raw(reqwest::Method::POST, "/api/v1/amapi/run-v3");
        let built = req.build().unwrap();
        assert_eq!(
            built.url().as_str(),
            "http://127.0.0.1:9999/api/v1/amapi/run-v3"
        );
    }

    #[test]
    fn request_includes_token_header() {
        let client = CiderClient::new().with_token("my-secret");
        let req = client.request(reqwest::Method::GET, "/active");
        let built = req.build().unwrap();
        assert_eq!(built.headers().get("apptoken").unwrap(), "my-secret");
    }

    #[test]
    fn request_omits_token_header_when_none() {
        let client = CiderClient::new();
        let req = client.request(reqwest::Method::GET, "/active");
        let built = req.build().unwrap();
        assert!(built.headers().get("apptoken").is_none());
    }

    #[test]
    fn request_raw_includes_token_header() {
        let client = CiderClient::new().with_token("secret");
        let req = client.request_raw(reqwest::Method::POST, "/api/v1/amapi/run-v3");
        let built = req.build().unwrap();
        assert_eq!(built.headers().get("apptoken").unwrap(), "secret");
    }
}
