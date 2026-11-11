// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Playback endpoints of the v2 API.

use reqwest::Method;

use super::types::{
    CollectionBody, EnabledData, HrefBody, ItemBody, PreviousBody, RateData, RepeatData, SeekBody,
    StateData, UrlBody,
};
use super::{
    exec, fetch, AudioQuality, PlaybackSnapshot, PlaybackState, PlaybackTime, RepeatMode, V2,
};
use crate::client::CiderError;
use crate::types::NowPlaying;

/// Starting a collection resolves it against the catalog first, which can
/// take longer than the client's default request timeout.
const PLAY_COLLECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Playback endpoints. Created by [`V2::playback`].
#[derive(Debug, Clone, Copy)]
pub struct Playback<'a> {
    pub(super) v2: V2<'a>,
}

impl Playback<'_> {
    /// Get the full playback state in one request.
    ///
    /// Sends `GET /api/v2/playback`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn snapshot(&self) -> Result<PlaybackSnapshot, CiderError> {
        fetch(self.v2.req(Method::GET, "/playback")).await
    }

    /// Get the currently loaded track, or `None` if nothing is loaded.
    ///
    /// Sends `GET /api/v2/playback/now-playing`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn now_playing(&self) -> Result<Option<NowPlaying>, CiderError> {
        let value: serde_json::Value =
            fetch(self.v2.req(Method::GET, "/playback/now-playing")).await?;
        // With nothing loaded Cider returns an object without track metadata.
        if value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .is_none()
        {
            return Ok(None);
        }
        Ok(serde_json::from_value(value).ok())
    }

    /// Get the transport state.
    ///
    /// This endpoint only distinguishes playing from not playing; use
    /// [`snapshot`](Self::snapshot) to tell paused from stopped.
    ///
    /// Sends `GET /api/v2/playback/state`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn state(&self) -> Result<PlaybackState, CiderError> {
        let data: StateData = fetch(self.v2.req(Method::GET, "/playback/state")).await?;
        Ok(data.state)
    }

    /// Get the playback position.
    ///
    /// Sends `GET /api/v2/playback/time`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn time(&self) -> Result<PlaybackTime, CiderError> {
        fetch(self.v2.req(Method::GET, "/playback/time")).await
    }

    /// Get the audio quality of the current track.
    ///
    /// Sends `GET /api/v2/playback/audio-quality`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn audio_quality(&self) -> Result<AudioQuality, CiderError> {
        fetch(self.v2.req(Method::GET, "/playback/audio-quality")).await
    }

    /// Resume playback.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn play(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::POST, "/playback/play")).await
    }

    /// Pause playback.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn pause(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::POST, "/playback/pause")).await
    }

    /// Stop playback and unload the current track.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn stop(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::POST, "/playback/stop")).await
    }

    /// Toggle between playing and paused.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn toggle(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::POST, "/playback/toggle")).await
    }

    /// Skip to the next track.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn next(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::POST, "/playback/next")).await
    }

    /// Go to the previous track.
    ///
    /// By default Cider restarts the current track when it is past the first
    /// few seconds. Pass `force = true` to always jump to the previous track.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn previous(&self, force: bool) -> Result<(), CiderError> {
        exec(
            self.v2
                .req(Method::POST, "/playback/previous")
                .json(&PreviousBody { force }),
        )
        .await
    }

    /// Seek to a position in seconds. Negative values are clamped to `0`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn seek(&self, position_secs: f64) -> Result<(), CiderError> {
        exec(self.v2.req(Method::POST, "/playback/seek").json(&SeekBody {
            position: position_secs.max(0.0),
        }))
        .await
    }

    /// Play an item by type and ID (e.g. `"songs"`, `"1719861213"`).
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn play_item(&self, item_type: &str, id: &str) -> Result<(), CiderError> {
        exec(
            self.v2
                .req(Method::POST, "/playback/play-item")
                .json(&ItemBody { item_type, id }),
        )
        .await
    }

    /// Play an Apple Music URL.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn play_url(&self, url: &str) -> Result<(), CiderError> {
        exec(
            self.v2
                .req(Method::POST, "/playback/play-url")
                .json(&UrlBody { url }),
        )
        .await
    }

    /// Play an Apple Music API href (e.g. `"/v1/catalog/us/songs/1719861213"`).
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn play_href(&self, href: &str) -> Result<(), CiderError> {
        exec(
            self.v2
                .req(Method::POST, "/playback/play-href")
                .json(&HrefBody { href }),
        )
        .await
    }

    /// Replace the queue with an album, playlist or station and start playing.
    ///
    /// `shuffle` is ignored for stations. Pass `None` to leave it off.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with code `NO_PLAYABLE_TRACKS` if the
    /// collection has nothing playable, or another [`CiderError`] if the
    /// request fails.
    pub async fn play_collection(
        &self,
        item_type: &str,
        id: &str,
        shuffle: Option<bool>,
    ) -> Result<(), CiderError> {
        exec(
            self.v2
                .req(Method::POST, "/playback/play-collection")
                .timeout(PLAY_COLLECTION_TIMEOUT)
                .json(&CollectionBody {
                    item_type,
                    id,
                    shuffle,
                }),
        )
        .await
    }

    /// Get the playback rate (`1.0` = normal speed).
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn rate(&self) -> Result<f64, CiderError> {
        let data: RateData = fetch(self.v2.req(Method::GET, "/playback/rate")).await?;
        Ok(data.rate)
    }

    /// Set the playback rate. Values are clamped to `0.25..=4.0`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn set_rate(&self, rate: f64) -> Result<(), CiderError> {
        let rate = rate.clamp(0.25, 4.0);
        let _: RateData = fetch(
            self.v2
                .req(Method::PATCH, "/playback/rate")
                .json(&RateData { rate }),
        )
        .await?;
        Ok(())
    }

    /// Get the repeat mode.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn repeat(&self) -> Result<RepeatMode, CiderError> {
        let data: RepeatData = fetch(self.v2.req(Method::GET, "/playback/repeat")).await?;
        Ok(data.mode)
    }

    /// Set the repeat mode.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn set_repeat(&self, mode: RepeatMode) -> Result<(), CiderError> {
        let _: RepeatData = fetch(
            self.v2
                .req(Method::PUT, "/playback/repeat")
                .json(&RepeatData { mode }),
        )
        .await?;
        Ok(())
    }

    /// Cycle the repeat mode (none → one → all).
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn toggle_repeat(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::POST, "/playback/repeat/toggle")).await
    }

    /// Check whether shuffle is on.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn shuffle(&self) -> Result<bool, CiderError> {
        let data: EnabledData = fetch(self.v2.req(Method::GET, "/playback/shuffle")).await?;
        Ok(data.enabled)
    }

    /// Turn shuffle on or off.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn set_shuffle(&self, enabled: bool) -> Result<(), CiderError> {
        let _: EnabledData = fetch(
            self.v2
                .req(Method::PUT, "/playback/shuffle")
                .json(&EnabledData { enabled }),
        )
        .await?;
        Ok(())
    }

    /// Toggle shuffle.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn toggle_shuffle(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::POST, "/playback/shuffle/toggle")).await
    }

    /// Check whether autoplay is on.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn autoplay(&self) -> Result<bool, CiderError> {
        let data: EnabledData = fetch(self.v2.req(Method::GET, "/playback/autoplay")).await?;
        Ok(data.enabled)
    }

    /// Turn autoplay on or off.
    ///
    /// Turning it on makes Cider append suggested tracks to the queue;
    /// turning it off again does not remove them.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn set_autoplay(&self, enabled: bool) -> Result<(), CiderError> {
        let _: EnabledData = fetch(
            self.v2
                .req(Method::PUT, "/playback/autoplay")
                .json(&EnabledData { enabled }),
        )
        .await?;
        Ok(())
    }

    /// Toggle autoplay.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn toggle_autoplay(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::POST, "/playback/autoplay/toggle")).await
    }
}
