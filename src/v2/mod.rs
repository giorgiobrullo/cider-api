// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Client for the Cider v2 API (`/api/v2`), available on **Cider 4 and later**.
//!
//! The v1 methods on [`CiderClient`] keep working on every Cider version. The
//! v2 API adds explicit setters (instead of toggles), a paginated queue, a
//! one-shot playback snapshot, library browsing, playlist editing, lyrics,
//! audio settings, structured errors and scoped tokens.
//!
//! Reach it through [`CiderClient::v2`]:
//!
//! ```no_run
//! # async fn example() -> Result<(), cider_api::CiderError> {
//! use cider_api::{v2::RepeatMode, CiderClient};
//!
//! let client = CiderClient::new().with_token("your-api-token");
//!
//! // Find out whether this Cider speaks v2 at all.
//! if client.server_info().await?.is_none() {
//!     println!("Older Cider: only the v1 methods are available");
//!     return Ok(());
//! }
//!
//! let v2 = client.v2();
//! let snapshot = v2.playback().snapshot().await?;
//! println!("{:?} at {:.0}s", snapshot.state, snapshot.time.current_time);
//!
//! v2.playback().set_repeat(RepeatMode::All).await?;
//! let page = v2.queue().list(0, 20).await?;
//! println!("{} of {} queue entries", page.items.len(), page.total);
//! # Ok(())
//! # }
//! ```
//!
//! # Areas and scopes
//!
//! | Accessor | Scope | What it covers |
//! |---|---|---|
//! | [`V2::playback`] | `playback` | Transport, seek, rate, repeat, shuffle, autoplay |
//! | [`V2::queue`] | `queue` | Paginated queue, reordering, Smart Queue Optimizer |
//! | [`V2::audio`] | `audio` | Volume, crossfade, automix, Atmos, normalization, … |
//! | [`V2::library`] | `library` | Current-track status and rating, library browsing |
//! | [`V2::playlists`] | `library` | Creating and editing playlists, history |
//! | [`V2::catalog`] | `library` | Apple Music catalog search and lookups |
//! | [`V2::lyrics`] | `lyrics` | Lyrics, community variations, translations |
//! | [`V2::config`] | `config` | Read-only Cider settings |
//! | [`V2::client_tokens`] | `account` | `MusicKit` tokens |
//! | [`V2::plugins`] | `plugins` | Installing plugins |
//! | `V2::events` | none | Real-time events (cargo feature `events`) |
//!
//! A token created without scopes can call everything.
//!
//! # Errors
//!
//! On a Cider that predates v2, every method here returns
//! [`CiderError::Unsupported`]. API-level failures come back as
//! [`CiderError::Server`] with Cider's machine-readable `code`
//! (e.g. `"INSUFFICIENT_SCOPE"`, `"RPC_TIMEOUT"`, `"INVALID_REQUEST"`).
//!
//! Cider 4.0 closes the connection of any request it has not answered
//! within 30 seconds; that surfaces as [`CiderError::Interrupted`]. It only
//! matters for calls that wait on the user or on a slow backend:
//! [`V2::request_auth`], [`Plugins::install`] and
//! [`Lyrics::translations`].

mod audio;
mod catalog;
#[cfg(feature = "events")]
mod events;
mod library;
mod lyrics;
mod playback;
mod playlists;
mod plugins;
mod queue;
mod types;

use std::time::Duration;

use reqwest::header::RETRY_AFTER;
use reqwest::{Method, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;

use crate::client::{CiderClient, CiderError};

pub use audio::Audio;
pub use catalog::Catalog;
#[cfg(feature = "events")]
pub use events::{
    Event, EventStream, NowPlayingChange, NowPlayingStatus, PlaybackAnchor, PlaybackProgress,
    PlaybackStateChange, PlaylistChange, PlaylistOperation, Transition, TransitionMode,
};
pub use library::Library;
pub use lyrics::Lyrics;
pub use playback::Playback;
pub use playlists::Playlists;
pub use plugins::{InstalledPlugin, Plugins};
pub use queue::Queue;
pub use types::{
    AtmosDeviceMode, AtmosState, AtmosUpdate, AudioQuality, AuthGrant, AutomixState, AutomixUpdate,
    CaptureMode, CaptureState, CaptureUpdate, CatalogOptions, ClientInfo, CrossfadeState,
    CrossfadeUpdate, DeviceAudioConfig, ExclusiveCapture, LibraryAlbum, LibraryArtist,
    LibraryPlaylist, LibrarySong, LibraryStatus, ListeningMode, ListeningModeState, LocalPlayback,
    LyricLine, LyricWord, LyricsLanguage, LyricsOptions, LyricsProvider, LyricsSource,
    LyricsTranslations, LyricsVariationInfo, LyricsVariations, MusicKitTokens, NewPlaylist,
    NormalizationState, NormalizationUpdate, Page, PlaybackSnapshot, PlaybackState, PlaybackTime,
    PlaylistHistoryEntry, PlaylistHistoryTrack, PlaylistUpdate, PlaylistWrite, QueueEntry,
    QueuePage, QueuePosition, RadioMode, RadioState, RadioUpdate, RecentlyAddedItem, RepeatMode,
    Scope, SearchOptions, SmartQueueStatus, SongLyrics, TrackRef, TrackSelection, TrackType,
    TransitionExclusions, TransitionExclusionsUpdate, TranslatedLine, TranslationCredit,
};
use types::{AuthRequest, Empty, Envelope, ErrorBody, ErrorEnvelope, PageData, PageMeta};

/// Cider waits up to two minutes for the user to answer an auth prompt.
const AUTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(130);

/// Timeout for endpoints that round-trip to Apple Music before answering.
pub(crate) const REMOTE_TIMEOUT: Duration = Duration::from_secs(35);

/// Handle for the Cider v2 API. Created by [`CiderClient::v2`].
#[derive(Debug, Clone, Copy)]
pub struct V2<'a> {
    client: &'a CiderClient,
}

impl<'a> V2<'a> {
    pub(crate) fn new(client: &'a CiderClient) -> Self {
        Self { client }
    }

    pub(crate) fn req(self, method: Method, path: &str) -> RequestBuilder {
        self.client.request_raw(method, &format!("/api/v2{path}"))
    }

    /// Playback control and state (scope: `playback`).
    #[must_use]
    pub fn playback(&self) -> Playback<'a> {
        Playback { v2: *self }
    }

    /// Queue inspection and editing (scope: `queue`).
    #[must_use]
    pub fn queue(&self) -> Queue<'a> {
        Queue { v2: *self }
    }

    /// Volume and audio processing settings (scope: `audio`).
    #[must_use]
    pub fn audio(&self) -> Audio<'a> {
        Audio { v2: *self }
    }

    /// Current-track library actions and library browsing (scope: `library`).
    #[must_use]
    pub fn library(&self) -> Library<'a> {
        Library { v2: *self }
    }

    /// Playlist creation and editing (scope: `library`).
    #[must_use]
    pub fn playlists(&self) -> Playlists<'a> {
        Playlists { v2: *self }
    }

    /// Apple Music catalog search and lookups (scope: `library`).
    #[must_use]
    pub fn catalog(&self) -> Catalog<'a> {
        Catalog { v2: *self }
    }

    /// Lyrics, community variations and translations (scope: `lyrics`).
    #[must_use]
    pub fn lyrics(&self) -> Lyrics<'a> {
        Lyrics { v2: *self }
    }

    /// Plugin installation (scope: `plugins`).
    #[must_use]
    pub fn plugins(&self) -> Plugins<'a> {
        Plugins { v2: *self }
    }

    /// Connect to Cider's real-time event channel.
    ///
    /// Needs no API token and must be called from within a Tokio runtime.
    /// See [`EventStream`] and [`Event`].
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::NotReachable`] if Cider is not running, or
    /// [`CiderError::Events`] if the handshake fails.
    #[cfg(feature = "events")]
    pub async fn events(&self) -> Result<EventStream, CiderError> {
        events::connect(self.client.base_url()).await
    }

    /// Get information about the running Cider instance.
    ///
    /// Sends `GET /api/v2/client/info`. Prefer
    /// [`CiderClient::server_info`] when you only want to detect v2 support.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Unsupported`] on a Cider without the v2 API.
    pub async fn client_info(&self) -> Result<ClientInfo, CiderError> {
        fetch(self.req(Method::GET, "/client/info")).await
    }

    /// Get the `MusicKit` developer token, user token and storefront
    /// (scope: `account`).
    ///
    /// These are the user's Apple Music credentials — handle them with care.
    /// For most uses [`V2::catalog`] is safer: Cider injects the tokens
    /// server-side and the caller never sees them.
    ///
    /// Sends `GET /api/v2/client/tokens`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn client_tokens(&self) -> Result<MusicKitTokens, CiderError> {
        fetch(self.req(Method::GET, "/client/tokens")).await
    }

    /// Get a read-only snapshot of Cider's settings (scope: `config`).
    ///
    /// API tokens are redacted and plugin settings are excluded. The shape
    /// follows Cider's own config file and is not stable across versions,
    /// so it is returned as raw JSON.
    ///
    /// Sends `GET /api/v2/config`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn config(&self) -> Result<serde_json::Value, CiderError> {
        fetch(self.req(Method::GET, "/config")).await
    }

    /// Get one subtree of Cider's settings by dot-notation path, e.g.
    /// `"audio.crossfade"` (scope: `config`).
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with code `CONFIG_PATH_NOT_FOUND` if
    /// nothing exists at `path`, or `INVALID_REQUEST` if the path is malformed.
    pub async fn config_path(&self, path: &str) -> Result<serde_json::Value, CiderError> {
        fetch(self.req(Method::GET, "/config").query(&[("path", path)])).await
    }

    /// Ask the user to grant this application an API token.
    ///
    /// Cider shows an approval prompt naming `app_name` and the requested
    /// `scopes`; this call resolves once the user answers. No existing token
    /// is needed.
    ///
    /// Tell your user to answer promptly: Cider 4.0 keeps the prompt open
    /// for two minutes but closes this request's connection after 30
    /// seconds, so a slower answer ends in [`CiderError::Interrupted`] and
    /// an approval given after that point is lost. The prompt stays on
    /// screen until it is answered or expires, and new requests fail with
    /// `AUTH_REQUEST_BUSY` in the meantime.
    ///
    /// Sends `POST /api/v2/auth/request`.
    ///
    /// ```no_run
    /// # async fn example() -> Result<(), cider_api::CiderError> {
    /// use cider_api::{v2::Scope, CiderClient};
    ///
    /// let grant = CiderClient::new()
    ///     .v2()
    ///     .request_auth("My App", &[Scope::Playback, Scope::Queue])
    ///     .await?;
    /// let client = CiderClient::new().with_token(grant.token);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with one of these codes:
    ///
    /// - `AUTH_REQUEST_DENIED` — the user declined.
    /// - `AUTH_REQUEST_TIMEOUT` — the user did not answer in time.
    /// - `AUTH_REQUEST_BUSY` — another prompt is already open.
    /// - `AUTH_REQUEST_COOLDOWN` — a request was made less than 5 seconds ago.
    /// - `AUTH_REQUEST_BANNED` — three denials within five minutes; requests
    ///   are refused for 15 minutes.
    ///
    /// For the last two, `retry_after` says how long to wait.
    ///
    /// Returns [`CiderError::Interrupted`] if Cider closes the connection
    /// before the user answers (see above).
    pub async fn request_auth(
        &self,
        app_name: &str,
        scopes: &[Scope],
    ) -> Result<AuthGrant, CiderError> {
        self.request_auth_inner(app_name, None, scopes).await
    }

    /// Like [`request_auth`](Self::request_auth), but also shows an app icon
    /// in the prompt. `app_image` must be an `https://` or `data:image/…` URL.
    ///
    /// # Errors
    ///
    /// Same as [`request_auth`](Self::request_auth).
    pub async fn request_auth_with_image(
        &self,
        app_name: &str,
        app_image: &str,
        scopes: &[Scope],
    ) -> Result<AuthGrant, CiderError> {
        self.request_auth_inner(app_name, Some(app_image), scopes)
            .await
    }

    async fn request_auth_inner(
        &self,
        app_name: &str,
        app_image: Option<&str>,
        scopes: &[Scope],
    ) -> Result<AuthGrant, CiderError> {
        let body = AuthRequest {
            app_name,
            app_image,
            scopes,
        };
        fetch(
            self.req(Method::POST, "/auth/request")
                .timeout(AUTH_REQUEST_TIMEOUT)
                .json(&body),
        )
        .await
    }
}

// ─── Response handling ───────────────────────────────────────────────────────

pub(crate) async fn send<T: DeserializeOwned>(
    req: RequestBuilder,
) -> Result<Envelope<T>, CiderError> {
    let resp = req.send().await.map_err(|e| {
        // A request that reached Cider but never got a response: the server
        // closed the socket, which Cider 4.0 does after 30 s of waiting.
        if e.is_request() && !e.is_timeout() && !e.is_connect() {
            CiderError::Interrupted(e)
        } else {
            CiderError::Http(e)
        }
    })?;
    let status = resp.status();
    if status.is_success() {
        return Ok(resp.json().await?);
    }

    let retry_after = resp
        .headers()
        .get(RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_secs);
    let text = resp.text().await.unwrap_or_default();

    Err(match serde_json::from_str::<ErrorEnvelope>(&text) {
        Ok(ErrorEnvelope {
            error: ErrorBody::Structured { code, message },
        }) => CiderError::Server {
            status: status.as_u16(),
            code,
            message,
            retry_after,
        },
        Ok(ErrorEnvelope {
            error: ErrorBody::Plain(code),
        }) => match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => CiderError::Unauthorized,
            _ => CiderError::Server {
                status: status.as_u16(),
                message: code.clone(),
                code,
                retry_after,
            },
        },
        // Every v2 route answers errors with a JSON envelope, so a bare 404
        // means the route itself does not exist: this Cider predates v2.
        Err(_) if status == StatusCode::NOT_FOUND => CiderError::Unsupported,
        Err(_) if status == StatusCode::UNAUTHORIZED => CiderError::Unauthorized,
        Err(_) => CiderError::Api(format!("Unexpected response (HTTP {})", status.as_u16())),
    })
}

pub(crate) async fn fetch<T: DeserializeOwned>(req: RequestBuilder) -> Result<T, CiderError> {
    Ok(send::<T>(req).await?.data)
}

pub(crate) async fn exec(req: RequestBuilder) -> Result<(), CiderError> {
    send::<Empty>(req).await.map(|_| ())
}

/// Fetch one page of a `{ data: { items }, meta: { offset, limit, total } }`
/// listing. `limit` is clamped to `1..=max_limit`.
pub(crate) async fn fetch_page<T: DeserializeOwned>(
    req: RequestBuilder,
    offset: u32,
    limit: u32,
    max_limit: u32,
) -> Result<Page<T>, CiderError> {
    let limit = limit.clamp(1, max_limit);
    let env: Envelope<PageData<T>> = send(
        req.timeout(REMOTE_TIMEOUT)
            .query(&[("offset", offset), ("limit", limit)]),
    )
    .await?;
    let meta = env.meta.unwrap_or(PageMeta {
        offset,
        limit,
        total: 0,
    });
    Ok(Page {
        items: env.data.items,
        offset: meta.offset,
        limit: meta.limit,
        total: meta.total,
    })
}
