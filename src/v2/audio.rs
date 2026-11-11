// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Audio endpoints of the v2 API.

use reqwest::Method;

use super::types::{ListeningModeBody, VolumeData};
use super::{
    fetch, AtmosState, AtmosUpdate, AutomixState, AutomixUpdate, CaptureState, CaptureUpdate,
    CrossfadeState, CrossfadeUpdate, ListeningMode, ListeningModeState, NormalizationState,
    NormalizationUpdate, RadioState, RadioUpdate, V2,
};
use crate::client::CiderError;

/// Audio endpoints. Created by [`V2::audio`].
///
/// The `set_*` methods take a partial update — only the fields you set are
/// changed — and return the resulting state. Changes are saved to Cider's
/// settings, exactly as if made in its preferences window.
#[derive(Debug, Clone, Copy)]
pub struct Audio<'a> {
    pub(super) v2: V2<'a>,
}

impl Audio<'_> {
    /// Get the volume (`0.0` = muted, `1.0` = full).
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn volume(&self) -> Result<f32, CiderError> {
        let data: VolumeData = fetch(self.v2.req(Method::GET, "/audio/volume")).await?;
        Ok(data.volume)
    }

    /// Set the volume. Values are clamped to `0.0..=1.0`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn set_volume(&self, volume: f32) -> Result<(), CiderError> {
        let volume = volume.clamp(0.0, 1.0);
        let _: VolumeData = fetch(
            self.v2
                .req(Method::PATCH, "/audio/volume")
                .json(&VolumeData { volume }),
        )
        .await?;
        Ok(())
    }

    /// Get the crossfade settings.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn crossfade(&self) -> Result<CrossfadeState, CiderError> {
        fetch(self.v2.req(Method::GET, "/audio/crossfade")).await
    }

    /// Change the crossfade settings and return the new state.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with code `INVALID_REQUEST` if the
    /// update is empty or a value is out of range, or another [`CiderError`]
    /// if the request fails.
    pub async fn set_crossfade(
        &self,
        update: &CrossfadeUpdate,
    ) -> Result<CrossfadeState, CiderError> {
        fetch(self.v2.req(Method::PATCH, "/audio/crossfade").json(update)).await
    }

    /// Get the automix settings.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn automix(&self) -> Result<AutomixState, CiderError> {
        fetch(self.v2.req(Method::GET, "/audio/automix")).await
    }

    /// Change the automix settings and return the new state.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with code `INVALID_REQUEST` if the
    /// update is empty or a value is out of range, or another [`CiderError`]
    /// if the request fails.
    pub async fn set_automix(&self, update: &AutomixUpdate) -> Result<AutomixState, CiderError> {
        fetch(self.v2.req(Method::PATCH, "/audio/automix").json(update)).await
    }

    /// Get the Dolby Atmos settings.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn atmos(&self) -> Result<AtmosState, CiderError> {
        fetch(self.v2.req(Method::GET, "/audio/atmos")).await
    }

    /// Change the Dolby Atmos settings and return the new state.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with code `INVALID_REQUEST` if the
    /// update is empty or a value is out of range, or another [`CiderError`]
    /// if the request fails.
    pub async fn set_atmos(&self, update: &AtmosUpdate) -> Result<AtmosState, CiderError> {
        fetch(self.v2.req(Method::PATCH, "/audio/atmos").json(update)).await
    }

    /// Get the Listening Mode preset.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn listening_mode(&self) -> Result<ListeningModeState, CiderError> {
        fetch(self.v2.req(Method::GET, "/audio/listening-mode")).await
    }

    /// Switch the Listening Mode preset and return the new state.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn set_listening_mode(
        &self,
        mode: ListeningMode,
    ) -> Result<ListeningModeState, CiderError> {
        fetch(
            self.v2
                .req(Method::PATCH, "/audio/listening-mode")
                .json(&ListeningModeBody { mode }),
        )
        .await
    }

    /// Get the Radio Sound settings.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn radio_mode(&self) -> Result<RadioState, CiderError> {
        fetch(self.v2.req(Method::GET, "/audio/radio-mode")).await
    }

    /// Change the Radio Sound settings and return the new state.
    ///
    /// Requires Cider 4.0.40 or later.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with code `INVALID_REQUEST` if the
    /// update is empty or a value is out of range, or another [`CiderError`]
    /// if the request fails.
    pub async fn set_radio_mode(&self, update: &RadioUpdate) -> Result<RadioState, CiderError> {
        fetch(self.v2.req(Method::PATCH, "/audio/radio-mode").json(update)).await
    }

    /// Get the host audio capture settings.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn capture(&self) -> Result<CaptureState, CiderError> {
        fetch(self.v2.req(Method::GET, "/audio/capture")).await
    }

    /// Change the host audio capture settings and return the new state.
    ///
    /// Host audio capture exists only on Windows; elsewhere this returns
    /// [`CiderError::Server`] with code `CAPABILITY_UNAVAILABLE`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with code `INVALID_REQUEST` if the
    /// update is empty or a value is out of range, or another [`CiderError`]
    /// if the request fails.
    pub async fn set_capture(&self, update: &CaptureUpdate) -> Result<CaptureState, CiderError> {
        fetch(self.v2.req(Method::PATCH, "/audio/capture").json(update)).await
    }

    /// Get the volume normalization settings.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn normalization(&self) -> Result<NormalizationState, CiderError> {
        fetch(self.v2.req(Method::GET, "/audio/normalization")).await
    }

    /// Change the volume normalization settings and return the new state.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with code `INVALID_REQUEST` if the
    /// update is empty or a value is out of range, or another [`CiderError`]
    /// if the request fails.
    pub async fn set_normalization(
        &self,
        update: &NormalizationUpdate,
    ) -> Result<NormalizationState, CiderError> {
        fetch(
            self.v2
                .req(Method::PATCH, "/audio/normalization")
                .json(update),
        )
        .await
    }
}
