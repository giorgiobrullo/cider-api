// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Request and response types for the Cider v2 API (`/api/v2`, Cider 4+).

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::{Artwork, NowPlaying, QueueItem};

// ─── Wire envelopes (internal) ───────────────────────────────────────────────

/// Success envelope: `{ "data": ..., "meta": ... }`.
#[derive(Debug, Deserialize)]
pub(crate) struct Envelope<T> {
    pub data: T,
    #[serde(default)]
    pub meta: Option<PageMeta>,
}

/// Pagination metadata attached to list responses.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct PageMeta {
    pub offset: u32,
    pub limit: u32,
    pub total: u32,
}

/// Error envelope. Cider sends `{ "error": { "code", "message" } }` for API
/// errors and `{ "error": "UNAUTHORIZED_APP_TOKEN" }` for token failures.
#[derive(Debug, Deserialize)]
pub(crate) struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum ErrorBody {
    Structured { code: String, message: String },
    Plain(String),
}

/// `data` of a paginated listing.
#[derive(Debug, Deserialize)]
pub(crate) struct PageData<T> {
    #[serde(default = "Vec::new")]
    pub items: Vec<T>,
}

/// One page of a paginated listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    /// Items in this page.
    pub items: Vec<T>,

    /// Offset of the first item in this page.
    pub offset: u32,

    /// Page size that was requested.
    pub limit: u32,

    /// Total number of items across all pages.
    pub total: u32,
}

impl<T> Page<T> {
    /// Offset of the next page, or `None` if this is the last one.
    #[must_use]
    pub fn next_offset(&self) -> Option<u32> {
        let len = u32::try_from(self.items.len()).unwrap_or(u32::MAX);
        let next = self.offset.saturating_add(len);
        (len > 0 && next < self.total).then_some(next)
    }
}

/// Empty `data` object returned by command endpoints.
#[derive(Debug, Deserialize)]
pub(crate) struct Empty {}

// ─── Client / auth ───────────────────────────────────────────────────────────

/// Information about the running Cider instance (`GET /client/info`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    /// UI framework the client is built on, or `"unknown"`.
    #[serde(default)]
    pub framework: String,

    /// Operating system identifier (e.g. `"darwin"`, `"win32"`, `"linux"`).
    #[serde(default)]
    pub platform: Option<String>,

    /// Operating system version.
    #[serde(default)]
    pub os_version: Option<String>,

    /// Port the API server is listening on.
    #[serde(default)]
    pub port: Option<u16>,

    /// `true` for production builds.
    #[serde(default)]
    pub production: bool,

    /// Version string reported by the client, if any.
    ///
    /// This is not the Cider release number: Cider 4.0.40 reports `"2.0.0"`.
    /// Do not use it to decide which features are available.
    #[serde(default)]
    pub version: Option<String>,
}

/// Permission scope carried by a v2 API token.
///
/// Tokens created without scopes (or with authentication disabled in Cider)
/// have access to everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Scope {
    /// Playback control and state.
    Playback,
    /// Queue inspection and editing.
    Queue,
    /// Library, playlists, ratings and Apple Music catalog access.
    Library,
    /// Volume and audio processing settings.
    Audio,
    /// Apple Music account tokens.
    Account,
    /// Plugin management.
    Plugins,
    /// Client configuration.
    Config,
    /// Lyrics.
    Lyrics,
}

#[derive(Debug, Serialize)]
pub(crate) struct AuthRequest<'a> {
    pub app_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_image: Option<&'a str>,
    pub scopes: &'a [Scope],
}

/// Token granted by the user through [`V2::request_auth`](super::V2::request_auth).
///
/// The `Debug` output redacts the token.
#[derive(Clone, Serialize, Deserialize)]
pub struct AuthGrant {
    /// The API token. Pass it to
    /// [`CiderClient::with_token`](crate::CiderClient::with_token).
    pub token: String,

    /// Scopes the user actually approved (may be fewer than requested).
    ///
    /// Kept as strings so scopes added by newer Cider versions still parse.
    #[serde(default)]
    pub scopes: Vec<String>,

    /// Browser origins allowed to call the API with this token. Empty unless
    /// the request came from a non-loopback web page.
    #[serde(default)]
    pub origins: Vec<String>,
}

impl fmt::Debug for AuthGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthGrant")
            .field("token", &"<redacted>")
            .field("scopes", &self.scopes)
            .field("origins", &self.origins)
            .finish()
    }
}

/// Apple Music credentials held by Cider (`GET /client/tokens`).
///
/// The `Debug` output redacts both tokens.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicKitTokens {
    /// `MusicKit` developer token (JWT).
    #[serde(default)]
    pub developer_token: String,

    /// The user's Apple Music token.
    #[serde(default)]
    pub user_token: String,

    /// Storefront (country) code, e.g. `"us"`.
    #[serde(default)]
    pub storefront: String,
}

impl fmt::Debug for MusicKitTokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MusicKitTokens")
            .field("developer_token", &"<redacted>")
            .field("user_token", &"<redacted>")
            .field("storefront", &self.storefront)
            .finish()
    }
}

// ─── Playback ────────────────────────────────────────────────────────────────

/// Transport state of the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PlaybackState {
    /// Audio is playing.
    Playing,
    /// Playback is paused.
    Paused,
    /// Nothing is loaded, or playback was stopped.
    Stopped,
    /// A state this crate version does not know about.
    #[serde(other)]
    Unknown,
}

/// Repeat mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    /// Repeat off.
    #[serde(rename = "none")]
    Off,
    /// Repeat the current track.
    One,
    /// Repeat the whole queue.
    All,
}

/// Playback position (`GET /playback/time`). All values are in seconds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackTime {
    /// Current position.
    #[serde(default)]
    pub current_time: f64,

    /// Total duration of the current track.
    #[serde(default)]
    pub duration: f64,

    /// Time left in the current track.
    #[serde(default)]
    pub remaining: f64,
}

/// Full playback state in one request (`GET /playback`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    /// Transport state.
    pub state: PlaybackState,

    /// The loaded track, or `None` if nothing is loaded.
    ///
    /// The live-position fields of [`NowPlaying`] (`current_playback_time`,
    /// `remaining_time`, `shuffle_mode`, `repeat_mode`) are not populated
    /// here — use the snapshot's own fields instead.
    #[serde(default)]
    pub now_playing: Option<NowPlaying>,

    /// Playback position.
    #[serde(default)]
    pub time: PlaybackTime,

    /// Volume, `0.0..=1.0`.
    #[serde(default)]
    pub volume: f32,

    /// Repeat mode.
    pub repeat_mode: RepeatMode,

    /// `true` if shuffle is on.
    #[serde(default)]
    pub shuffle_mode: bool,

    /// `true` if autoplay is on.
    #[serde(default)]
    pub autoplay: bool,

    /// Playback rate (`1.0` = normal speed).
    #[serde(default = "default_rate")]
    pub rate: f64,
}

fn default_rate() -> f64 {
    1.0
}

/// Output device capabilities reported by the audio backend.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAudioConfig {
    /// Number of output channels.
    #[serde(default)]
    pub channel_count: Option<u32>,

    /// Output sample rate in Hz.
    #[serde(default)]
    pub sample_rate: Option<f64>,

    /// `true` if the device can play Dolby Atmos.
    #[serde(default)]
    pub supports_atmos: bool,

    /// Codecs the device supports.
    #[serde(default)]
    pub supported_codecs: Vec<String>,
}

/// Format details for a locally stored file being played.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPlayback {
    /// Bitrate in kbps.
    #[serde(default)]
    pub bitrate_kbps: Option<f64>,

    /// Sample rate in Hz.
    #[serde(default)]
    pub sample_rate: Option<f64>,

    /// Bit depth.
    #[serde(default)]
    pub bits_per_sample: Option<u32>,

    /// Channel count.
    #[serde(default)]
    pub channels: Option<u32>,

    /// Codec name.
    #[serde(default)]
    pub codec: Option<String>,

    /// `true` if the file is lossless.
    #[serde(default)]
    pub lossless: bool,
}

/// Audio quality of the current track (`GET /playback/audio-quality`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioQuality {
    /// Raw flavor identifier (e.g. `"256"`, `"atmos"`, `"unk"`).
    #[serde(default)]
    pub flavor: String,

    /// Human-readable label (e.g. `"AAC 256kbps"`, `"Dolby Atmos"`).
    #[serde(default)]
    pub flavor_label: String,

    /// Output device capabilities, when the backend exposes them.
    #[serde(default)]
    pub device_audio_config: Option<DeviceAudioConfig>,

    /// Local file details, when a local file is playing.
    #[serde(default)]
    pub local_playback: Option<LocalPlayback>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StateData {
    pub state: PlaybackState,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RateData {
    pub rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RepeatData {
    pub mode: RepeatMode,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct EnabledData {
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct VolumeData {
    pub volume: f32,
}

#[derive(Debug, Serialize)]
pub(crate) struct PreviousBody {
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct SeekBody {
    pub position: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct ItemBody<'a> {
    #[serde(rename = "type")]
    pub item_type: &'a str,
    pub id: &'a str,
}

#[derive(Debug, Serialize)]
pub(crate) struct CollectionBody<'a> {
    #[serde(rename = "type")]
    pub item_type: &'a str,
    pub id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shuffle: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UrlBody<'a> {
    pub url: &'a str,
}

#[derive(Debug, Serialize)]
pub(crate) struct HrefBody<'a> {
    pub href: &'a str,
}

// ─── Queue ───────────────────────────────────────────────────────────────────

/// One entry of the v2 queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    /// The queued track.
    pub track: QueueItem,

    /// Where the entry came from (opaque; shape depends on the provider).
    #[serde(default)]
    pub source: Option<serde_json::Value>,

    /// The album/playlist/station the entry was queued from (opaque).
    #[serde(default)]
    pub container_context: Option<serde_json::Value>,
}

/// A page of the queue (`GET /queue`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuePage {
    /// Entries in this page.
    pub items: Vec<QueueEntry>,

    /// Index of the currently playing entry within the whole queue.
    pub position: u32,

    /// Offset of the first entry in this page.
    pub offset: u32,

    /// Page size that was requested.
    pub limit: u32,

    /// Total number of entries in the queue.
    pub total: u32,
}

/// Queue position without the entries (`GET /queue/position`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuePosition {
    /// Index of the currently playing entry.
    #[serde(default)]
    pub position: u32,

    /// Total number of entries in the queue.
    #[serde(default)]
    pub total: u32,
}

/// Smart Queue Optimizer status (`GET /queue/smart-optimizer`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SmartQueueStatus {
    /// `true` if the optimizer is on.
    #[serde(default)]
    pub enabled: bool,

    /// `true` while the optimizer is reordering the queue.
    #[serde(default)]
    pub busy: bool,

    /// Number of entries still waiting to be processed.
    #[serde(default)]
    pub pending: u32,

    /// Status message, if any.
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QueueData {
    #[serde(default)]
    pub items: Vec<QueueEntry>,
    #[serde(default)]
    pub position: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct IndexBody {
    pub index: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct MoveBody {
    pub from: u32,
    pub to: u32,
}

// ─── Library ─────────────────────────────────────────────────────────────────

/// Library status of the current track (`GET /library/now-playing/status`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    /// `true` if the track is in the user's library.
    #[serde(default)]
    pub in_library: bool,

    /// `-1` = disliked, `0` = unrated, `1` = loved.
    #[serde(default)]
    pub rating: i8,
}

#[derive(Debug, Serialize)]
pub(crate) struct RatingBody {
    pub rating: i8,
}

// ─── Audio settings ──────────────────────────────────────────────────────────

/// Content types that crossfade and automix leave alone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionExclusions {
    /// Skip transitions when playing an album.
    #[serde(default)]
    pub albums: bool,
    /// Skip transitions when playing a playlist.
    #[serde(default)]
    pub playlists: bool,
    /// Skip transitions inside DJ mixes.
    #[serde(default)]
    pub dj_mixes: bool,
    /// Skip transitions inside live albums.
    #[serde(default)]
    pub live_albums: bool,
}

/// Partial update of [`TransitionExclusions`]; `None` fields are left as is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionExclusionsUpdate {
    /// See [`TransitionExclusions::albums`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub albums: Option<bool>,
    /// See [`TransitionExclusions::playlists`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playlists: Option<bool>,
    /// See [`TransitionExclusions::dj_mixes`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dj_mixes: Option<bool>,
    /// See [`TransitionExclusions::live_albums`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_albums: Option<bool>,
}

/// Crossfade settings (`GET /audio/crossfade`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct CrossfadeState {
    /// The user's setting.
    #[serde(default)]
    pub enabled: bool,
    /// Whether crossfade is actually in effect right now (another feature,
    /// such as automix, can override it).
    #[serde(default)]
    pub effective_enabled: bool,
    /// Crossfade length in seconds.
    #[serde(default)]
    pub duration_sec: f64,
    /// Crossfade on manual skips too.
    #[serde(default)]
    pub skip_enabled: bool,
    /// Tempo sensitivity, `0.0..=1.0`.
    #[serde(default)]
    pub tempo_sensitivity: f64,
    /// Loudness sensitivity, `0.0..=1.0`.
    #[serde(default)]
    pub loudness_sensitivity: f64,
    /// Content types that are not crossfaded.
    #[serde(default)]
    pub exclusions: TransitionExclusions,
}

/// Partial update for [`Audio::set_crossfade`](super::Audio::set_crossfade).
/// Set at least one field.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossfadeUpdate {
    /// Turn crossfade on or off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Crossfade length in seconds, `0.0..=30.0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_sec: Option<f64>,
    /// Crossfade on manual skips too.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_enabled: Option<bool>,
    /// Tempo sensitivity, `0.0..=1.0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tempo_sensitivity: Option<f64>,
    /// Loudness sensitivity, `0.0..=1.0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loudness_sensitivity: Option<f64>,
    /// Exclusions to change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusions: Option<TransitionExclusionsUpdate>,
}

/// Automix settings (`GET /audio/automix`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct AutomixState {
    /// The user's setting.
    #[serde(default)]
    pub enabled: bool,
    /// Whether automix is actually in effect right now.
    #[serde(default)]
    pub effective_enabled: bool,
    /// Spectral separation, `0.0..=1.0`.
    #[serde(default)]
    pub spectral_separation: f64,
    /// Avoid mixing over vocals.
    #[serde(default)]
    pub vocal_guard: bool,
    /// Align tempo between tracks.
    #[serde(default)]
    pub tempo_align: bool,
    /// Loudness-normalization ramp in milliseconds.
    #[serde(default)]
    pub normalize_duration_ms: f64,
    /// Content types that are not automixed.
    #[serde(default)]
    pub exclusions: TransitionExclusions,
}

/// Partial update for [`Audio::set_automix`](super::Audio::set_automix).
/// Set at least one field.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomixUpdate {
    /// Turn automix on or off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Spectral separation, `0.0..=1.0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectral_separation: Option<f64>,
    /// Avoid mixing over vocals.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vocal_guard: Option<bool>,
    /// Align tempo between tracks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tempo_align: Option<bool>,
    /// Loudness-normalization ramp in milliseconds, `0.0..=30000.0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalize_duration_ms: Option<f64>,
    /// Exclusions to change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusions: Option<TransitionExclusionsUpdate>,
}

/// How Dolby Atmos is rendered on one output device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum AtmosDeviceMode {
    /// Atmos disabled on this device.
    Off,
    /// Pass the bitstream through to the device.
    Passthrough,
    /// Standard multichannel rendering.
    Standard,
    /// Binaural rendering for headphones.
    Binaural,
    /// A mode this crate version does not know about.
    #[serde(other)]
    Unknown,
}

/// Dolby Atmos settings (`GET /audio/atmos`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtmosState {
    /// `true` if Atmos playback is on.
    #[serde(default)]
    pub enabled: bool,
    /// `true` if binaural rendering is on.
    #[serde(default)]
    pub binaural: bool,
    /// Per-device overrides, keyed by device identifier.
    #[serde(default)]
    pub device_preferences: HashMap<String, AtmosDeviceMode>,
}

/// Partial update for [`Audio::set_atmos`](super::Audio::set_atmos).
/// Set at least one field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtmosUpdate {
    /// Turn Atmos on or off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Turn binaural rendering on or off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binaural: Option<bool>,
    /// Per-device overrides to merge into the existing ones.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_preferences: Option<HashMap<String, AtmosDeviceMode>>,
}

/// Listening Mode preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ListeningMode {
    /// No processing.
    Off,
    /// Tuned for listening while gaming.
    Game,
    /// Reduces listening fatigue.
    Antifatigue,
    /// Warm, relaxed sound.
    Cozy,
    /// A preset this crate version does not know about.
    #[serde(other)]
    Unknown,
}

/// Listening Mode state (`GET /audio/listening-mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListeningModeState {
    /// Active preset.
    pub mode: ListeningMode,
    /// `false` if the current audio backend cannot apply Listening Mode.
    #[serde(default = "default_true")]
    pub available: bool,
}

fn default_true() -> bool {
    true
}

/// Radio Sound preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum RadioMode {
    /// No processing.
    Off,
    FmRadio,
    FmHits,
    FmRock,
    FmUrban,
    FmSmooth,
    HdRadio,
    HdHits,
    HdSmooth,
    HdClassical,
    /// A preset this crate version does not know about.
    #[serde(other)]
    Unknown,
}

/// Radio Sound state (`GET /audio/radio-mode`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadioState {
    /// Active preset.
    pub mode: RadioMode,
    /// Amplification, `0.0..=100.0`.
    #[serde(default)]
    pub amplify: f64,
    /// Processing amount, `0.0..=100.0`.
    #[serde(default)]
    pub processing: f64,
    /// Stereo blend, `0.0..=100.0`.
    #[serde(default)]
    pub stereo_blend: f64,
    /// Limit bandwidth like a real broadcast.
    #[serde(default)]
    pub bandwidth_limit: bool,
    /// `false` if the current audio backend cannot apply Radio Sound.
    #[serde(default = "default_true")]
    pub available: bool,
}

/// Partial update for [`Audio::set_radio_mode`](super::Audio::set_radio_mode).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadioUpdate {
    /// Preset to switch to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<RadioMode>,
    /// Amplification, `0.0..=100.0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amplify: Option<f64>,
    /// Processing amount, `0.0..=100.0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing: Option<f64>,
    /// Stereo blend, `0.0..=100.0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stereo_blend: Option<f64>,
    /// Limit bandwidth like a real broadcast.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bandwidth_limit: Option<bool>,
}

/// Host audio capture mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum CaptureMode {
    /// Capture disabled.
    Off,
    /// Shared-mode capture.
    Capture,
    /// Exclusive-mode output.
    Exclusive,
    /// A mode this crate version does not know about.
    #[serde(other)]
    Unknown,
}

/// Exclusive-mode output format. `0` means "let the device decide".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExclusiveCapture {
    /// Sample rate in Hz: `0`, `48000`, `88200`, `96000`, `176400` or `192000`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    /// Bit depth: `0`, `16`, `24` or `32`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<u32>,
}

/// Host audio capture state (`GET /audio/capture`). Windows only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureState {
    /// `false` on platforms without host audio capture (macOS, Linux).
    #[serde(default)]
    pub supported: bool,
    /// Active mode.
    pub mode: CaptureMode,
    /// Gain in dB.
    #[serde(default)]
    pub gain_db: f64,
    /// Enable capture automatically when one of `auto_apps` starts.
    #[serde(default)]
    pub auto_enable: bool,
    /// Applications that trigger automatic capture.
    #[serde(default)]
    pub auto_apps: Vec<String>,
    /// Exclusive-mode output format.
    #[serde(default)]
    pub exclusive: ExclusiveCapture,
}

/// Partial update for [`Audio::set_capture`](super::Audio::set_capture).
/// Set at least one field.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureUpdate {
    /// Mode to switch to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<CaptureMode>,
    /// Gain in dB, `-24.0..=12.0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gain_db: Option<f64>,
    /// Enable capture automatically.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_enable: Option<bool>,
    /// Applications that trigger automatic capture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_apps: Option<Vec<String>>,
    /// Exclusive-mode output format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusive: Option<ExclusiveCapture>,
}

/// Volume normalization state (`GET /audio/normalization`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizationState {
    /// `true` if normalization is on.
    #[serde(default)]
    pub enabled: bool,
    /// Target loudness in LUFS.
    #[serde(default)]
    pub target_lufs: f64,
}

/// Partial update for
/// [`Audio::set_normalization`](super::Audio::set_normalization).
/// Set at least one field.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizationUpdate {
    /// Turn normalization on or off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Target loudness in LUFS, `-18.0..=-8.0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_lufs: Option<f64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ListeningModeBody {
    pub mode: ListeningMode,
}

// ─── Library browsing ────────────────────────────────────────────────────────
//
// Artwork URLs in these listings are ready to use: Cider has already replaced
// the `{w}`/`{h}` placeholders.

/// A playlist in the user's library.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPlaylist {
    /// Library playlist ID (e.g. `"p.rXAJKBgiDaM4vDx"`).
    pub id: String,
    /// Playlist name.
    #[serde(default)]
    pub name: String,
    /// Description, possibly empty.
    #[serde(default)]
    pub description: Option<String>,
    /// `true` if the user can edit this playlist.
    #[serde(default)]
    pub can_edit: bool,
    /// Number of tracks, when Apple Music reports it.
    #[serde(default)]
    pub track_count: Option<u32>,
    /// Cover artwork.
    #[serde(default)]
    pub artwork: Option<Artwork>,
}

/// An album in the user's library.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAlbum {
    /// Library album ID (e.g. `"l.vncTZKl"`).
    pub id: String,
    /// Album name.
    #[serde(default)]
    pub name: String,
    /// Artist name.
    #[serde(default)]
    pub artist_name: Option<String>,
    /// Number of tracks from this album in the library.
    #[serde(default)]
    pub track_count: Option<u32>,
    /// Release date (`YYYY-MM-DD`).
    #[serde(default)]
    pub release_date: Option<String>,
    /// Cover artwork.
    #[serde(default)]
    pub artwork: Option<Artwork>,
}

/// An artist in the user's library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryArtist {
    /// Library artist ID (e.g. `"r.lTdTiW2"`).
    pub id: String,
    /// Artist name.
    #[serde(default)]
    pub name: String,
    /// Artist artwork, usually absent for library artists.
    #[serde(default)]
    pub artwork: Option<Artwork>,
}

/// A song in the user's library, a library album or a library playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySong {
    /// Song ID (e.g. `"i.eoD8Vl8ukLZp6kM"`).
    pub id: String,
    /// Song name.
    #[serde(default)]
    pub name: String,
    /// Artist name.
    #[serde(default)]
    pub artist_name: Option<String>,
    /// Album name.
    #[serde(default)]
    pub album_name: Option<String>,
    /// Duration in milliseconds.
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// Track number on the album.
    #[serde(default)]
    pub track_number: Option<u32>,
    /// Cover artwork.
    #[serde(default)]
    pub artwork: Option<Artwork>,
}

/// An entry of the "Recently Added" shelf: an album, playlist or other
/// resource, told apart by [`item_type`](Self::item_type).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentlyAddedItem {
    /// Resource type (e.g. `"library-albums"`, `"library-playlists"`).
    #[serde(default, rename = "type")]
    pub item_type: Option<String>,
    /// Resource ID.
    pub id: String,
    /// Name.
    #[serde(default)]
    pub name: String,
    /// Artist name, for albums.
    #[serde(default)]
    pub artist_name: Option<String>,
    /// Number of tracks.
    #[serde(default)]
    pub track_count: Option<u32>,
    /// Release date (`YYYY-MM-DD`), for albums.
    #[serde(default)]
    pub release_date: Option<String>,
    /// Cover artwork.
    #[serde(default)]
    pub artwork: Option<Artwork>,
}

// ─── Playlists ───────────────────────────────────────────────────────────────

/// Kind of track referenced by a [`TrackRef`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrackType {
    /// A song already in the library (`i.…` IDs).
    LibrarySongs,
    /// A music video already in the library.
    LibraryMusicVideos,
    /// A catalog song. Adding it imports it into the library.
    Songs,
    /// A catalog music video. Adding it imports it into the library.
    MusicVideos,
}

/// Reference to a track, for playlist edits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackRef {
    /// Track ID. Letters, digits, `.`, `_` and `-` only.
    pub id: String,
    /// Track kind.
    #[serde(rename = "type")]
    pub track_type: TrackType,
}

impl TrackRef {
    /// Reference a catalog song by its Apple Music ID.
    #[must_use]
    pub fn song(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            track_type: TrackType::Songs,
        }
    }

    /// Reference a song that is already in the library.
    #[must_use]
    pub fn library_song(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            track_type: TrackType::LibrarySongs,
        }
    }
}

/// Parameters for [`Playlists::create`](super::Playlists::create).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewPlaylist {
    /// Playlist name, 1–200 characters.
    pub name: String,
    /// Description, up to 1000 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Initial tracks, at most 100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracks: Option<Vec<TrackRef>>,
    /// Library folder to create the playlist in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Must be `true` when `tracks` contains catalog tracks, because adding
    /// them also adds them to the user's library.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_library_import: Option<bool>,
}

impl NewPlaylist {
    /// A new, empty playlist called `name`.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
}

/// Partial update for [`Playlists::update`](super::Playlists::update).
/// Set at least one field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistUpdate {
    /// New name, 1–200 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// New description, up to 1000 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Make the playlist public or private.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_public: Option<bool>,
}

/// Which tracks to remove from a playlist. Set at least one of the two.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TrackSelection {
    /// Track IDs to remove (at most 500).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<String>>,
    /// 0-based positions to remove (at most 500).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indices: Option<Vec<u32>>,
}

/// Result of a playlist track edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistWrite {
    /// Number of tracks in the playlist after the edit.
    #[serde(default)]
    pub track_count: u32,
    /// Restore point taken just before the edit. Pass it to
    /// [`Playlists::restore`](super::Playlists::restore) to undo.
    #[serde(default)]
    pub snapshot_at: Option<String>,
    /// Number of tracks added (only for `add_tracks`).
    #[serde(default)]
    pub added: Option<u32>,
    /// Number of tracks removed (only for `remove_tracks`).
    #[serde(default)]
    pub removed: Option<u32>,
}

/// A track recorded in a playlist history snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistHistoryTrack {
    /// Track ID.
    #[serde(default)]
    pub id: String,
    /// Song name.
    #[serde(default)]
    pub name: String,
    /// Artist name.
    #[serde(default)]
    pub artist_name: String,
    /// Album name.
    #[serde(default)]
    pub album_name: String,
    /// Duration in milliseconds.
    #[serde(default)]
    pub duration_in_millis: u64,
    /// Artwork URL.
    #[serde(default)]
    pub artwork: String,
}

/// One snapshot of a playlist's track list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistHistoryEntry {
    /// Snapshot identifier, to pass to
    /// [`Playlists::restore`](super::Playlists::restore).
    pub timestamp: String,
    /// Number of tracks in the snapshot.
    #[serde(default)]
    pub track_count: u32,
    /// The tracks, in order.
    #[serde(default)]
    pub tracks: Vec<PlaylistHistoryTrack>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct IdData {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct HistoryData {
    #[serde(default)]
    pub entries: Vec<PlaylistHistoryEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AddTracksBody<'a> {
    pub tracks: &'a [TrackRef],
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub allow_library_import: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReplaceTracksBody<'a> {
    pub tracks: &'a [TrackRef],
    pub expected_track_count: u32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub allow_library_import: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct TimestampBody<'a> {
    pub timestamp: &'a str,
}

// ─── Lyrics ──────────────────────────────────────────────────────────────────

/// Lyrics provider to request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum LyricsProvider {
    /// Apple Music.
    AppleMusic,
    /// Musixmatch.
    Musixmatch,
    /// `NetEase` Cloud Music.
    Netease,
    /// Lyrics supplied by the user.
    User,
}

impl LyricsProvider {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AppleMusic => "apple-music",
            Self::Musixmatch => "musixmatch",
            Self::Netease => "netease",
            Self::User => "user",
        }
    }
}

/// Options for lyrics requests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LyricsOptions {
    /// Force a provider instead of Cider's configured order. Ignored by
    /// [`Lyrics::current`](super::Lyrics::current) and
    /// [`Lyrics::variation`](super::Lyrics::variation).
    pub provider: Option<LyricsProvider>,
    /// Include word-level timing in [`LyricLine::words`] when available.
    pub words: bool,
    /// Include the raw TTML document in [`SongLyrics::ttml`].
    pub ttml: bool,
}

/// Where a set of lyrics came from.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSource {
    /// Provider display name (e.g. `"Apple Music"`).
    #[serde(default)]
    pub provider: String,
    /// Timing granularity (e.g. `"Line"`, `"Word"`, `"None"`).
    #[serde(default)]
    pub timing_type: String,
    /// Language tag (e.g. `"en-US"`).
    #[serde(default)]
    pub language: Option<String>,
    /// Contributor, for community submissions.
    #[serde(default)]
    pub contributor_name: Option<String>,
}

/// One word of a lyric line, with timing in seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricWord {
    /// Start time.
    #[serde(default)]
    pub start: f64,
    /// End time.
    #[serde(default)]
    pub end: f64,
    /// The word.
    #[serde(default)]
    pub text: String,
    /// `true` for background vocals.
    #[serde(default)]
    pub is_background: bool,
}

/// One line of lyrics, with timing in seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct LyricLine {
    /// Start time.
    #[serde(default)]
    pub start: f64,
    /// End time.
    #[serde(default)]
    pub end: f64,
    /// The line. Empty for instrumental gaps.
    #[serde(default)]
    pub text: String,
    /// `true` for an instrumental gap.
    #[serde(default)]
    pub empty: bool,
    /// Song section (e.g. `"Verse"`, `"Chorus"`).
    #[serde(default)]
    pub song_part: Option<String>,
    /// `true` if sung by a second vocalist.
    #[serde(default)]
    pub is_duet: bool,
    /// `true` for background vocals.
    #[serde(default)]
    pub is_background: bool,
    /// Word-level timing, when requested and available.
    #[serde(default)]
    pub words: Option<Vec<LyricWord>>,
}

/// Lyrics for a song or a community variation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongLyrics {
    /// Song ID (absent for community variations).
    #[serde(default)]
    pub id: Option<String>,
    /// Variation ID (only for community variations).
    #[serde(default)]
    pub variation_id: Option<u32>,
    /// Where the lyrics came from.
    #[serde(default)]
    pub source: LyricsSource,
    /// Songwriters.
    #[serde(default)]
    pub writers: Vec<String>,
    /// The lines, in order.
    #[serde(default)]
    pub lines: Vec<LyricLine>,
    /// Raw TTML document, when requested and available.
    #[serde(default)]
    pub ttml: Option<String>,
}

/// A community lyric submission for a track.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsVariationInfo {
    /// ID to pass to [`Lyrics::variation`](super::Lyrics::variation).
    pub variation_id: u32,
    /// Contributor name.
    #[serde(default)]
    pub contributor_name: Option<String>,
    /// Language tag.
    #[serde(default)]
    pub language: String,
    /// Timing granularity.
    #[serde(default)]
    pub sync_type: String,
    /// How often this variation has been used.
    #[serde(default)]
    pub access_count: u64,
    /// Track name as submitted.
    #[serde(default)]
    pub track_name: String,
    /// Artist name as submitted.
    #[serde(default)]
    pub artist_name: String,
}

/// Community lyric submissions for a track.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsVariations {
    /// Track the submissions belong to.
    #[serde(default)]
    pub track_id: String,
    /// The submissions.
    #[serde(default)]
    pub variations: Vec<LyricsVariationInfo>,
}

/// Attribution for a translation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationCredit {
    /// Translation source.
    #[serde(default)]
    pub source: String,
    /// `true` if the source requires attribution.
    #[serde(default)]
    pub attributed: bool,
    /// Attribution text to display.
    #[serde(default)]
    pub credit_text: Option<String>,
}

/// One translated lyric line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslatedLine {
    /// Original line.
    #[serde(default)]
    pub line: String,
    /// Translated line.
    #[serde(default)]
    pub translation: String,
}

/// Translated lyrics for a track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsTranslations {
    /// Target language.
    #[serde(default)]
    pub language: String,
    /// Attribution, if any.
    #[serde(default)]
    pub credit: Option<TranslationCredit>,
    /// The translated lines.
    #[serde(default)]
    pub translations: Vec<TranslatedLine>,
}

/// A language lyrics can be translated into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsLanguage {
    /// Language code to pass to
    /// [`Lyrics::translations`](super::Lyrics::translations).
    pub value: String,
    /// Display name.
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LanguagesData {
    #[serde(default)]
    pub languages: Vec<LyricsLanguage>,
}

// ─── Catalog ─────────────────────────────────────────────────────────────────

/// Options for [`Catalog::search`](super::Catalog::search).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchOptions {
    /// Resource types to search, e.g. `["songs", "albums"]`. Defaults to
    /// songs, albums, artists and playlists. Also accepted: `music-videos`,
    /// `stations`, `curators`, `apple-curators`.
    pub types: Vec<String>,
    /// Results per type, `1..=25`. Defaults to 25.
    pub limit: Option<u32>,
    /// Offset into the results.
    pub offset: Option<u32>,
    /// Language tag for localized results (e.g. `"en-GB"`).
    pub language: Option<String>,
    /// Two-letter storefront. Defaults to the user's own.
    pub storefront: Option<String>,
}

/// Options for [`Catalog::get`](super::Catalog::get).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogOptions {
    /// Relationships to include, e.g. `["albums", "artists"]`.
    pub include: Vec<String>,
    /// Views to include, e.g. `["top-songs"]`.
    pub views: Vec<String>,
    /// Language tag for localized results.
    pub language: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&Scope::Playback).unwrap(),
            "\"playback\""
        );
        assert_eq!(serde_json::to_string(&Scope::Lyrics).unwrap(), "\"lyrics\"");
    }

    #[test]
    fn error_body_parses_both_shapes() {
        let e: ErrorEnvelope =
            serde_json::from_str(r#"{"error":{"code":"RPC_TIMEOUT","message":"slow"}}"#).unwrap();
        assert!(matches!(e.error, ErrorBody::Structured { ref code, .. } if code == "RPC_TIMEOUT"));

        let e: ErrorEnvelope =
            serde_json::from_str(r#"{"error":"UNAUTHORIZED_APP_TOKEN"}"#).unwrap();
        assert!(matches!(e.error, ErrorBody::Plain(ref s) if s == "UNAUTHORIZED_APP_TOKEN"));
    }

    #[test]
    fn snapshot_parses_with_nothing_loaded() {
        let s: PlaybackSnapshot = serde_json::from_str(
            r#"{"state":"stopped","nowPlaying":null,
                "time":{"currentTime":0,"duration":0,"remaining":0},
                "volume":0.5,"repeatMode":"none","shuffleMode":false,
                "autoplay":true,"rate":1}"#,
        )
        .unwrap();
        assert_eq!(s.state, PlaybackState::Stopped);
        assert!(s.now_playing.is_none());
        assert!(s.autoplay);
    }

    #[test]
    fn secrets_are_redacted_in_debug() {
        let grant: AuthGrant =
            serde_json::from_str(r#"{"token":"s3cret","scopes":["playback"]}"#).unwrap();
        assert!(!format!("{grant:?}").contains("s3cret"));
        let tokens: MusicKitTokens = serde_json::from_str(
            r#"{"developerToken":"devtok","userToken":"usertok","storefront":"it"}"#,
        )
        .unwrap();
        let dbg = format!("{tokens:?}");
        assert!(!dbg.contains("devtok") && !dbg.contains("usertok") && dbg.contains("it"));
    }

    #[test]
    fn unknown_enum_values_do_not_break_parsing() {
        let s: RadioState = serde_json::from_str(r#"{"mode":"somethingnew","amplify":1}"#).unwrap();
        assert_eq!(s.mode, RadioMode::Unknown);
        let s: RadioState = serde_json::from_str(r#"{"mode":"fmhits"}"#).unwrap();
        assert_eq!(s.mode, RadioMode::FmHits);
        assert!(s.available);
    }

    #[test]
    fn updates_serialize_only_set_fields() {
        let u = CrossfadeUpdate {
            duration_sec: Some(4.0),
            ..Default::default()
        };
        assert_eq!(serde_json::to_string(&u).unwrap(), r#"{"durationSec":4.0}"#);
        let t = TrackRef::library_song("i.abc");
        assert_eq!(
            serde_json::to_string(&t).unwrap(),
            r#"{"id":"i.abc","type":"library-songs"}"#
        );
    }

    #[test]
    fn page_next_offset() {
        let p = Page {
            items: vec![1, 2],
            offset: 0,
            limit: 2,
            total: 5,
        };
        assert_eq!(p.next_offset(), Some(2));
        let p = Page {
            items: vec![5],
            offset: 4,
            limit: 2,
            total: 5,
        };
        assert_eq!(p.next_offset(), None);
        let p: Page<u8> = Page {
            items: vec![],
            offset: 0,
            limit: 2,
            total: 5,
        };
        assert_eq!(p.next_offset(), None);
    }

    #[test]
    fn queue_entry_tolerates_placeholder_track() {
        let e: QueueEntry = serde_json::from_str(
            r#"{"track":{"id":"","type":""},"source":null,"containerContext":null}"#,
        )
        .unwrap();
        assert_eq!(e.track.id.as_deref(), Some(""));
        assert!(e.source.is_none());
    }
}
