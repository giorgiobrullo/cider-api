// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Lyrics endpoints of the v2 API.

use std::time::Duration;

use reqwest::{Method, RequestBuilder};

use super::library::encode;
use super::types::LanguagesData;
use super::{
    fetch, LyricsLanguage, LyricsOptions, LyricsTranslations, LyricsVariations, SongLyrics, V2,
};
use crate::client::CiderError;

/// Cider gives lyrics providers up to a minute to answer.
const LYRICS_TIMEOUT: Duration = Duration::from_secs(65);

/// Lyrics endpoints. Created by [`V2::lyrics`].
///
/// # Errors
///
/// Besides the usual failures, these methods return [`CiderError::Server`]
/// with `LYRICS_NOT_FOUND`, `NO_CURRENT_TRACK`, `LYRICS_TIMEOUT`,
/// `LYRICS_VARIATION_NOT_FOUND` or `LYRICS_TRANSLATIONS_NOT_FOUND`.
#[derive(Debug, Clone, Copy)]
pub struct Lyrics<'a> {
    pub(super) v2: V2<'a>,
}

fn with_options(req: RequestBuilder, options: LyricsOptions, provider: bool) -> RequestBuilder {
    let mut query: Vec<(&str, &str)> = vec![
        ("words", if options.words { "true" } else { "false" }),
        ("ttml", if options.ttml { "true" } else { "false" }),
    ];
    if let (true, Some(p)) = (provider, options.provider) {
        query.push(("provider", p.as_str()));
    }
    req.timeout(LYRICS_TIMEOUT).query(&query)
}

impl Lyrics<'_> {
    /// Get the lyrics of the current track.
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn current(&self, options: LyricsOptions) -> Result<SongLyrics, CiderError> {
        fetch(with_options(
            self.v2.req(Method::GET, "/lyrics/current"),
            options,
            false,
        ))
        .await
    }

    /// Get the lyrics of a song by its catalog ID.
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn get(
        &self,
        song_id: &str,
        options: LyricsOptions,
    ) -> Result<SongLyrics, CiderError> {
        let path = format!("/lyrics/{}", encode(song_id));
        fetch(with_options(self.v2.req(Method::GET, &path), options, true)).await
    }

    /// List the community lyric submissions for a song.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with code `LYRICS_NOT_FOUND` if the
    /// song has none. See also the [type-level docs](Self#errors).
    pub async fn variations(&self, song_id: &str) -> Result<LyricsVariations, CiderError> {
        let path = format!("/lyrics/{}/variations", encode(song_id));
        fetch(self.v2.req(Method::GET, &path).timeout(LYRICS_TIMEOUT)).await
    }

    /// Get one community lyric submission.
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn variation(
        &self,
        song_id: &str,
        variation_id: u32,
        options: LyricsOptions,
    ) -> Result<SongLyrics, CiderError> {
        let path = format!("/lyrics/{}/variations/{variation_id}", encode(song_id));
        fetch(with_options(
            self.v2.req(Method::GET, &path),
            options,
            false,
        ))
        .await
    }

    /// Get a translation of a song's lyrics.
    ///
    /// `language` is an ISO 639-1 code from [`languages`](Self::languages).
    /// Translations are generated on demand: the first request for a song
    /// can fail with `LYRICS_TRANSLATIONS_NOT_FOUND`, or with
    /// [`CiderError::Interrupted`] when generating it takes Cider longer
    /// than 30 seconds, and succeed on a later attempt.
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn translations(
        &self,
        song_id: &str,
        language: &str,
    ) -> Result<LyricsTranslations, CiderError> {
        let path = format!("/lyrics/{}/translations", encode(song_id));
        fetch(
            self.v2
                .req(Method::GET, &path)
                .timeout(LYRICS_TIMEOUT)
                .query(&[("language", language)]),
        )
        .await
    }

    /// List the languages lyrics can be translated into.
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn languages(&self) -> Result<Vec<LyricsLanguage>, CiderError> {
        let data: LanguagesData = fetch(self.v2.req(Method::GET, "/lyrics/languages")).await?;
        Ok(data.languages)
    }
}
