// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Types for the Cider REST API.
//!
//! This module contains all request and response types used by
//! [`CiderClient`](crate::CiderClient). Response types use `#[serde(default)]`
//! on fields that may be absent so deserialization succeeds even when the API
//! omits them (e.g. radio stations may omit `artist_name`).
//!
//! The response shapes match the [Cider RPC documentation](https://cider.sh/docs/client/rpc).

use serde::{Deserialize, Serialize};

// ─── Response wrapper ────────────────────────────────────────────────────────

/// Generic wrapper for Cider API JSON responses.
///
/// Most endpoints return `{ "status": "ok", ...fields }`. The inner payload is
/// flattened so its fields sit alongside `status`.
///
/// # Example (JSON)
///
/// ```json
/// { "status": "ok", "is_playing": true }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse<T> {
    /// Status string, typically `"ok"`.
    pub status: String,

    /// Endpoint-specific payload, flattened into the same JSON object.
    #[serde(flatten)]
    pub data: T,
}

// ─── Common types ────────────────────────────────────────────────────────────

/// Artwork metadata for a track, album, or station.
///
/// The `url` field may contain `{w}` and `{h}` placeholders for the desired
/// image dimensions. Use [`Artwork::url_for_size`] to get a ready-to-use URL.
///
/// Color fields (`text_color1`–`text_color4`, `bg_color`) are hex color strings
/// present on certain container artwork (e.g. radio stations).
///
/// # Examples
///
/// ```
/// # use cider_api::Artwork;
/// let art = Artwork {
///     width: 600,
///     height: 600,
///     url: "https://example.com/img/{w}x{h}bb.jpg".into(),
///     ..Default::default()
/// };
/// assert_eq!(
///     art.url_for_size(300),
///     "https://example.com/img/300x300bb.jpg"
/// );
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artwork {
    /// Image width in pixels.
    #[serde(default)]
    pub width: u32,

    /// Image height in pixels.
    #[serde(default)]
    pub height: u32,

    /// URL template — may contain `{w}` and `{h}` size placeholders.
    #[serde(default)]
    pub url: String,

    /// Primary text color (hex, e.g. `"eaccc1"`). Present on station artwork.
    #[serde(default)]
    pub text_color1: Option<String>,

    /// Secondary text color (hex). Present on station artwork.
    #[serde(default)]
    pub text_color2: Option<String>,

    /// Tertiary text color (hex). Present on station artwork.
    #[serde(default)]
    pub text_color3: Option<String>,

    /// Quaternary text color (hex). Present on station artwork.
    #[serde(default)]
    pub text_color4: Option<String>,

    /// Background color (hex, e.g. `"0c0e0d"`). Present on station artwork.
    #[serde(default)]
    pub bg_color: Option<String>,

    /// Whether the artwork uses the Display P3 color space.
    #[serde(default)]
    pub has_p3: Option<bool>,
}

impl Artwork {
    /// Return the artwork URL with `{w}` and `{h}` replaced by `size`.
    ///
    /// If the URL has no placeholders the original URL is returned unchanged.
    #[must_use]
    pub fn url_for_size(&self, size: u32) -> String {
        let s = size.to_string();
        self.url.replace("{w}", &s).replace("{h}", &s)
    }
}

/// Play parameters identifying a playable item.
///
/// Every playable track, album, or station carries an `id` (Apple Music
/// catalog ID) and a `kind` (e.g. `"song"`, `"album"`, `"radioStation"`).
///
/// # Examples
///
/// ```
/// # use cider_api::PlayParams;
/// let pp = PlayParams { id: "1719861213".into(), kind: "song".into() };
/// assert_eq!(pp.id, "1719861213");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayParams {
    /// Apple Music catalog ID.
    pub id: String,

    /// Item kind — `"song"`, `"album"`, `"playlist"`, `"radioStation"`, etc.
    pub kind: String,
}

/// A track audio preview.
///
/// The `url` points to a short AAC preview clip hosted on Apple's CDN.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preview {
    /// Direct URL to the preview audio file.
    pub url: String,
}

// ─── Now Playing ─────────────────────────────────────────────────────────────

/// Currently playing track information returned by `GET /now-playing`.
///
/// This is an Apple Music API–style resource enriched with live playback
/// state (`current_playback_time`, `remaining_time`, `shuffle_mode`, etc.).
///
/// All fields use `#[serde(default)]` so deserialization succeeds even when
/// the API omits fields (e.g. radio stations may lack `artist_name`).
///
/// # Examples
///
/// ```
/// # use cider_api::NowPlaying;
/// # fn example(track: &NowPlaying) {
/// println!("{} — {} ({})", track.name, track.artist_name, track.album_name);
/// println!("Position: {:.1}s / {}ms", track.current_playback_time, track.duration_in_millis);
/// if let Some(id) = track.song_id() {
///     println!("Song ID: {id}");
/// }
/// println!("Artwork: {}", track.artwork_url(600));
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct NowPlaying {
    /// Song name.
    #[serde(default)]
    pub name: String,

    /// Artist name.
    #[serde(default)]
    pub artist_name: String,

    /// Album name.
    #[serde(default)]
    pub album_name: String,

    /// Artwork information.
    #[serde(default)]
    pub artwork: Artwork,

    /// Total duration in milliseconds.
    #[serde(default)]
    pub duration_in_millis: u64,

    // ── Identifiers ──

    /// Play parameters containing the song ID and kind.
    #[serde(default)]
    pub play_params: Option<PlayParams>,

    /// Apple Music web URL for the track.
    #[serde(default)]
    pub url: Option<String>,

    /// International Standard Recording Code.
    #[serde(default)]
    pub isrc: Option<String>,

    // ── Playback state (injected by Cider, not in the Apple Music catalog) ──

    /// Current playback position in seconds.
    #[serde(default)]
    pub current_playback_time: f64,

    /// Remaining playback time in seconds.
    #[serde(default)]
    pub remaining_time: f64,

    /// Shuffle mode — `0` = off, `1` = on.
    #[serde(default)]
    pub shuffle_mode: u8,

    /// Repeat mode — `0` = off, `1` = repeat one, `2` = repeat all.
    #[serde(default)]
    pub repeat_mode: u8,

    /// Whether the track is in the user's favorites.
    #[serde(default)]
    pub in_favorites: bool,

    /// Whether the track is in the user's library.
    #[serde(default)]
    pub in_library: bool,

    // ── Catalog metadata ──

    /// Genre names (e.g. `["Electronic", "Music"]`).
    #[serde(default)]
    pub genre_names: Vec<String>,

    /// Track number on the album.
    #[serde(default)]
    pub track_number: u32,

    /// Disc number on the album.
    #[serde(default)]
    pub disc_number: u32,

    /// Release date as an ISO-8601 string (e.g. `"2016-05-27T12:00:00Z"`).
    #[serde(default)]
    pub release_date: Option<String>,

    /// Audio locale code (e.g. `"en-US"`).
    #[serde(default)]
    pub audio_locale: Option<String>,

    /// Composer / songwriter name.
    #[serde(default)]
    pub composer_name: Option<String>,

    /// Whether the track has lyrics.
    #[serde(default)]
    pub has_lyrics: bool,

    /// Whether the track has time-synced (karaoke-style) lyrics.
    #[serde(default)]
    pub has_time_synced_lyrics: bool,

    /// Whether vocal attenuation (sing-along mode) is available.
    #[serde(default)]
    pub is_vocal_attenuation_allowed: bool,

    /// Legacy flag — replaced by [`is_apple_digital_master`](Self::is_apple_digital_master).
    #[serde(default)]
    pub is_mastered_for_itunes: bool,

    /// Whether the track is an Apple Digital Master (high-resolution master).
    #[serde(default)]
    pub is_apple_digital_master: bool,

    /// Audio traits (e.g. `["atmos", "lossless", "lossy-stereo", "spatial"]`).
    #[serde(default)]
    pub audio_traits: Vec<String>,

    /// Audio preview URLs.
    #[serde(default)]
    pub previews: Vec<Preview>,
}

impl NowPlaying {
    /// Get the song ID from [`play_params`](Self::play_params), if present.
    #[must_use]
    pub fn song_id(&self) -> Option<&str> {
        self.play_params.as_ref().map(|p| p.id.as_str())
    }

    /// Get the current playback position in milliseconds.
    ///
    /// Negative `current_playback_time` values (possible at seek boundaries)
    /// are clamped to zero.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn current_position_ms(&self) -> u64 {
        // max(0.0) guards against negative values; truncation is intentional.
        (self.current_playback_time.max(0.0) * 1000.0).round() as u64
    }

    /// Get the artwork URL at the specified square size (in pixels).
    ///
    /// Shorthand for `self.artwork.url_for_size(size)`.
    #[must_use]
    pub fn artwork_url(&self, size: u32) -> String {
        self.artwork.url_for_size(size)
    }
}

// ─── Queue types ─────────────────────────────────────────────────────────────

/// A single item in the Cider playback queue.
///
/// Returned as part of the array from `GET /queue`. The queue includes
/// history items, the currently playing track, and upcoming items. Use
/// [`QueueItem::is_current`] to identify the active track.
///
/// Most useful data lives in [`attributes`](Self::attributes). Top-level
/// fields like `asset_url`, `assets`, and `key_urls` are Apple Music
/// streaming internals.
///
/// # Examples
///
/// ```no_run
/// # use cider_api::{CiderClient, QueueItem};
/// # async fn example() -> Result<(), cider_api::CiderError> {
/// let queue = CiderClient::new().get_queue().await?;
///
/// // Find the currently playing item
/// if let Some(current) = queue.iter().find(|i| i.is_current()) {
///     if let Some(attrs) = &current.attributes {
///         println!("Now playing: {} — {}", attrs.name, attrs.artist_name);
///     }
/// }
///
/// // List upcoming tracks
/// let current_idx = queue.iter().position(|i| i.is_current()).unwrap_or(0);
/// for item in &queue[current_idx + 1..] {
///     if let Some(attrs) = &item.attributes {
///         println!("  Up next: {}", attrs.name);
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    /// Apple Music catalog ID for this item.
    #[serde(default)]
    pub id: Option<String>,

    /// Item type (e.g. `"song"`).
    #[serde(default, rename = "type")]
    pub item_type: Option<String>,

    /// HLS streaming URL for the asset.
    #[serde(default, rename = "assetURL")]
    pub asset_url: Option<String>,

    /// HLS metadata (opaque object).
    #[serde(default)]
    pub hls_metadata: Option<serde_json::Value>,

    /// Audio flavor / codec descriptor (e.g. `"28:ctrp256"`).
    #[serde(default)]
    pub flavor: Option<String>,

    /// Track metadata attributes.
    #[serde(default)]
    pub attributes: Option<QueueItemAttributes>,

    /// Playback type identifier.
    #[serde(default)]
    pub playback_type: Option<u32>,

    /// The container this item was queued from (e.g. a station or playlist).
    #[serde(default, rename = "_container")]
    pub container: Option<QueueContainer>,

    /// Context information about how this item was queued.
    #[serde(default, rename = "_context")]
    pub context: Option<QueueContext>,

    /// Playback state — `current == Some(2)` means currently playing.
    #[serde(default, rename = "_state")]
    pub state: Option<QueueItemState>,

    /// Song ID (may differ from `id` for library vs. catalog tracks).
    #[serde(default, rename = "_songId")]
    pub song_id: Option<String>,

    /// Available audio assets with different codec flavors and metadata.
    #[serde(default)]
    pub assets: Option<Vec<serde_json::Value>>,

    /// DRM key URLs for HLS playback.
    #[serde(default, rename = "keyURLs")]
    pub key_urls: Option<KeyUrls>,
}

impl QueueItem {
    /// Returns `true` if this is the currently playing item.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.state
            .as_ref()
            .and_then(|s| s.current)
            .is_some_and(|c| c == 2)
    }
}

/// Track attributes within a [`QueueItem`].
///
/// Contains the same catalog metadata as [`NowPlaying`] plus
/// live playback state injected by Cider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct QueueItemAttributes {
    /// Song name.
    #[serde(default)]
    pub name: String,

    /// Artist name.
    #[serde(default)]
    pub artist_name: String,

    /// Album name.
    #[serde(default)]
    pub album_name: String,

    /// Total duration in milliseconds.
    #[serde(default)]
    pub duration_in_millis: u64,

    // ── Identifiers ──

    /// Artwork information.
    #[serde(default)]
    pub artwork: Option<Artwork>,

    /// Play parameters containing the song ID and kind.
    #[serde(default)]
    pub play_params: Option<PlayParams>,

    /// Apple Music web URL for the track.
    #[serde(default)]
    pub url: Option<String>,

    /// International Standard Recording Code.
    #[serde(default)]
    pub isrc: Option<String>,

    // ── Catalog metadata ──

    /// Genre names.
    #[serde(default)]
    pub genre_names: Vec<String>,

    /// Track number on the album.
    #[serde(default)]
    pub track_number: u32,

    /// Disc number on the album.
    #[serde(default)]
    pub disc_number: u32,

    /// Release date as an ISO-8601 string.
    #[serde(default)]
    pub release_date: Option<String>,

    /// Audio locale code (e.g. `"en-US"`).
    #[serde(default)]
    pub audio_locale: Option<String>,

    /// Composer / songwriter name.
    #[serde(default)]
    pub composer_name: Option<String>,

    /// Whether the track has lyrics.
    #[serde(default)]
    pub has_lyrics: bool,

    /// Whether the track has time-synced lyrics.
    #[serde(default)]
    pub has_time_synced_lyrics: bool,

    /// Whether vocal attenuation is available.
    #[serde(default)]
    pub is_vocal_attenuation_allowed: bool,

    /// Legacy Mastered for iTunes flag.
    #[serde(default)]
    pub is_mastered_for_itunes: bool,

    /// Whether the track is an Apple Digital Master.
    #[serde(default)]
    pub is_apple_digital_master: bool,

    /// Audio traits (e.g. `["lossless", "lossy-stereo"]`).
    #[serde(default)]
    pub audio_traits: Vec<String>,

    /// Audio preview URLs.
    #[serde(default)]
    pub previews: Vec<Preview>,

    // ── Playback state (injected by Cider) ──

    /// Current playback position in seconds.
    #[serde(default)]
    pub current_playback_time: f64,

    /// Remaining playback time in seconds.
    #[serde(default)]
    pub remaining_time: f64,
}

/// Playback state of a [`QueueItem`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItemState {
    /// `2` indicates this is the currently playing item.
    #[serde(default)]
    pub current: Option<u8>,
}

/// The container (playlist, station, album) a queue item was sourced from.
///
/// Container `attributes` vary by type and are exposed as raw JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueContainer {
    /// Container ID (e.g. `"ra.cp-1055074639"`).
    #[serde(default)]
    pub id: Option<String>,

    /// Container type (e.g. `"stations"`, `"playlists"`, `"albums"`).
    #[serde(default, rename = "type")]
    pub container_type: Option<String>,

    /// Apple Music API href for the container.
    #[serde(default)]
    pub href: Option<String>,

    /// Display name / context label (e.g. `"now_playing"`).
    #[serde(default)]
    pub name: Option<String>,

    /// Container-specific attributes (varies by type).
    #[serde(default)]
    pub attributes: Option<serde_json::Value>,
}

/// Context metadata for a [`QueueItem`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueContext {
    /// Feature that queued this item (e.g. `"now_playing"`).
    #[serde(default)]
    pub feature_name: Option<String>,
}

/// DRM / streaming key URLs for HLS playback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyUrls {
    /// URL for the HLS `FairPlay` certificate bundle.
    #[serde(default, rename = "hls-key-cert-url")]
    pub hls_key_cert_url: Option<String>,

    /// URL for the HLS `FairPlay` license server.
    #[serde(default, rename = "hls-key-server-url")]
    pub hls_key_server_url: Option<String>,

    /// URL for the Widevine certificate.
    #[serde(default, rename = "widevine-cert-url")]
    pub widevine_cert_url: Option<String>,
}

// ─── Endpoint-specific response payloads ─────────────────────────────────────

/// Payload for `GET /is-playing`.
#[derive(Debug, Clone, Deserialize)]
pub struct IsPlayingResponse {
    /// `true` if music is currently playing.
    pub is_playing: bool,
}

/// Payload for `GET /now-playing`.
#[derive(Debug, Clone, Deserialize)]
pub struct NowPlayingResponse {
    /// Currently playing track info.
    pub info: NowPlaying,
}

/// Payload for `GET /volume`.
#[derive(Debug, Clone, Deserialize)]
pub struct VolumeResponse {
    /// Current volume level (`0.0`–`1.0`).
    pub volume: f32,
}

/// Payload for `GET /repeat-mode`.
#[derive(Debug, Clone, Deserialize)]
pub struct RepeatModeResponse {
    /// `0` = off, `1` = repeat one, `2` = repeat all.
    pub value: u8,
}

/// Payload for `GET /shuffle-mode`.
#[derive(Debug, Clone, Deserialize)]
pub struct ShuffleModeResponse {
    /// `0` = off, `1` = on.
    pub value: u8,
}

/// Payload for `GET /autoplay`.
#[derive(Debug, Clone, Deserialize)]
pub struct AutoplayResponse {
    /// `true` = autoplay enabled.
    pub value: bool,
}

// ─── Request bodies ──────────────────────────────────────────────────────────

/// Request body for `POST /play-url`.
#[derive(Debug, Clone, Serialize)]
pub struct PlayUrlRequest {
    /// Apple Music URL to play (e.g. `"https://music.apple.com/…"`).
    pub url: String,
}

/// Request body for `POST /play-item` / `POST /play-next` / `POST /play-later`.
#[derive(Debug, Clone, Serialize)]
pub struct PlayItemRequest {
    /// Item type (e.g. `"songs"`, `"albums"`, `"playlists"`).
    #[serde(rename = "type")]
    pub item_type: String,

    /// Apple Music catalog ID (must be a string, not a number).
    pub id: String,
}

/// Request body for `POST /play-item-href`.
#[derive(Debug, Clone, Serialize)]
pub struct PlayItemHrefRequest {
    /// Apple Music API href (e.g. `"/v1/catalog/ca/songs/1719861213"`).
    pub href: String,
}

/// Request body for `POST /seek`.
#[derive(Debug, Clone, Serialize)]
pub struct SeekRequest {
    /// Target position in **seconds**.
    pub position: f64,
}

/// Request body for `POST /volume`.
#[derive(Debug, Clone, Serialize)]
pub struct VolumeRequest {
    /// Target volume (`0.0`–`1.0`).
    pub volume: f32,
}

/// Request body for `POST /set-rating`.
#[derive(Debug, Clone, Serialize)]
pub struct RatingRequest {
    /// `-1` = dislike, `0` = unset, `1` = like.
    pub rating: i8,
}

/// Request body for `POST /queue/move-to-position`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueMoveRequest {
    /// Current 1-based index of the item to move.
    pub start_index: u32,

    /// Target 1-based index.
    pub destination_index: u32,

    /// If `true`, the response includes the updated queue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_queue: Option<bool>,
}

/// Request body for `POST /queue/remove-by-index`.
#[derive(Debug, Clone, Serialize)]
pub struct QueueRemoveRequest {
    /// 1-based index of the item to remove.
    pub index: u32,
}

/// Request body for `POST /api/v1/amapi/run-v3`.
#[derive(Debug, Clone, Serialize)]
pub struct AmApiRequest {
    /// Apple Music API path (e.g. `"/v1/catalog/ca/search?term=…"`).
    pub path: String,
}
