// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Library endpoints of the v2 API.

use reqwest::Method;

use super::types::RatingBody;
use super::{
    exec, fetch, fetch_page, LibraryAlbum, LibraryArtist, LibraryPlaylist, LibrarySong,
    LibraryStatus, Page, RecentlyAddedItem, REMOTE_TIMEOUT, V2,
};
use crate::client::CiderError;

/// Apple Music caps library pages at 100 items.
const LIBRARY_MAX_LIMIT: u32 = 100;

/// Library endpoints. Created by [`V2::library`].
///
/// The listing methods return one [`Page`] at a time; `limit` is clamped to
/// `1..=100`. Use [`Page::next_offset`] to walk through all pages.
#[derive(Debug, Clone, Copy)]
pub struct Library<'a> {
    pub(super) v2: V2<'a>,
}

impl Library<'_> {
    // ── Current track ──

    /// Get the library membership and rating of the current track.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn now_playing_status(&self) -> Result<LibraryStatus, CiderError> {
        fetch(
            self.v2
                .req(Method::GET, "/library/now-playing/status")
                .timeout(REMOTE_TIMEOUT),
        )
        .await
    }

    /// Add the current track to the library.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn add_now_playing(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::POST, "/library/now-playing/add")).await
    }

    /// Remove the current track from the library.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn remove_now_playing(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::DELETE, "/library/now-playing")).await
    }

    /// Mark the current track as loved.
    ///
    /// Cider also adds the track to the library if it was not in it;
    /// [`clear_rating`](Self::clear_rating) does not remove it again.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn love(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::POST, "/library/now-playing/love")).await
    }

    /// Mark the current track as disliked.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn dislike(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::POST, "/library/now-playing/dislike")).await
    }

    /// Remove the rating from the current track.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn clear_rating(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::DELETE, "/library/now-playing/rating")).await
    }

    /// Rate the current track: `-1` = dislike, `0` = unset, `1` = love.
    /// Values are clamped to `-1..=1`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn set_rating(&self, rating: i8) -> Result<(), CiderError> {
        let rating = rating.clamp(-1, 1);
        exec(
            self.v2
                .req(Method::PUT, "/library/now-playing/rating")
                .json(&RatingBody { rating }),
        )
        .await
    }

    // ── Browsing ──

    /// List the playlists in the library.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn playlists(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<Page<LibraryPlaylist>, CiderError> {
        let req = self.v2.req(Method::GET, "/library/playlists");
        fetch_page(req, offset, limit, LIBRARY_MAX_LIMIT).await
    }

    /// List the songs of a library playlist.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn playlist_songs(
        &self,
        playlist_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<Page<LibrarySong>, CiderError> {
        let req = self.v2.req(
            Method::GET,
            &format!("/library/playlists/{}/songs", encode(playlist_id)),
        );
        fetch_page(req, offset, limit, LIBRARY_MAX_LIMIT).await
    }

    /// List the albums in the library.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn albums(&self, offset: u32, limit: u32) -> Result<Page<LibraryAlbum>, CiderError> {
        let req = self.v2.req(Method::GET, "/library/albums");
        fetch_page(req, offset, limit, LIBRARY_MAX_LIMIT).await
    }

    /// List the songs of a library album.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn album_songs(
        &self,
        album_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<Page<LibrarySong>, CiderError> {
        let req = self.v2.req(
            Method::GET,
            &format!("/library/albums/{}/songs", encode(album_id)),
        );
        fetch_page(req, offset, limit, LIBRARY_MAX_LIMIT).await
    }

    /// List the artists in the library.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn artists(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<Page<LibraryArtist>, CiderError> {
        let req = self.v2.req(Method::GET, "/library/artists");
        fetch_page(req, offset, limit, LIBRARY_MAX_LIMIT).await
    }

    /// List the library albums of an artist.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn artist_albums(
        &self,
        artist_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<Page<LibraryAlbum>, CiderError> {
        let req = self.v2.req(
            Method::GET,
            &format!("/library/artists/{}/albums", encode(artist_id)),
        );
        fetch_page(req, offset, limit, LIBRARY_MAX_LIMIT).await
    }

    /// List the songs in the library.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn songs(&self, offset: u32, limit: u32) -> Result<Page<LibrarySong>, CiderError> {
        let req = self.v2.req(Method::GET, "/library/songs");
        fetch_page(req, offset, limit, LIBRARY_MAX_LIMIT).await
    }

    /// List recently added albums and playlists.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn recently_added(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<Page<RecentlyAddedItem>, CiderError> {
        let req = self.v2.req(Method::GET, "/library/recently-added");
        fetch_page(req, offset, limit, LIBRARY_MAX_LIMIT).await
    }
}

/// Percent-encode an ID for use as a single URL path segment.
pub(super) fn encode(segment: &str) -> String {
    use std::fmt::Write as _;

    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn encode_leaves_ids_alone_and_escapes_the_rest() {
        assert_eq!(encode("p.rXAJKBgiDaM4vDx"), "p.rXAJKBgiDaM4vDx");
        assert_eq!(encode("a/b?c d"), "a%2Fb%3Fc%20d");
    }
}
