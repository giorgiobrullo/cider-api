// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Playlist endpoints of the v2 API.

use std::time::Duration;

use reqwest::Method;

use super::library::encode;
use super::types::{AddTracksBody, HistoryData, IdData, ReplaceTracksBody, TimestampBody};
use super::{
    fetch, NewPlaylist, PlaylistHistoryEntry, PlaylistUpdate, PlaylistWrite, TrackRef,
    TrackSelection, REMOTE_TIMEOUT, V2,
};
use crate::client::CiderError;
use crate::v2::types::MoveBody;

/// Cider allows playlist writes up to two minutes.
const WRITE_TIMEOUT: Duration = Duration::from_secs(125);

/// Playlist endpoints. Created by [`V2::playlists`].
///
/// To list playlists and their songs, use [`V2::library`].
///
/// # Safety net
///
/// Before every edit Cider snapshots the playlist's track list. Each edit
/// returns that restore point in [`PlaylistWrite::snapshot_at`]; pass it to
/// [`restore`](Self::restore) to undo. Only the last few snapshots are kept
/// (10 by default). There is deliberately no way to delete a playlist.
///
/// # Consistency
///
/// Observed on Cider 4.0:
///
/// - [`PlaylistWrite::track_count`] is Cider's own count. Apple Music
///   applies the write a few seconds later, so
///   [`Library::playlist_songs`](super::Library::playlist_songs) can lag
///   behind it.
/// - A catalog track added with `allow_library_import` gets a library ID
///   (`i.…`) that stops working once the track is removed from the
///   playlist. Later writes that reference such an ID are accepted but
///   skip the track — this includes [`restore`](Self::restore) to a
///   snapshot that contains it. Reference tracks by catalog ID
///   ([`TrackRef::song`](super::TrackRef::song)) when in doubt.
///
/// # Errors
///
/// Besides the usual failures, edits return [`CiderError::Server`] with:
///
/// - `PLAYLIST_NOT_FOUND`, `PLAYLIST_NOT_EDITABLE`, `PLAYLIST_IS_SMART`
/// - `PLAYLIST_CONFLICT` — the playlist changed underneath the edit; retry.
/// - `CATALOG_IMPORT_NOT_ALLOWED` — catalog tracks need
///   `allow_library_import`.
/// - `INVALID_TRACK_REF`, `TRACK_COUNT_MISMATCH`, `HISTORY_ENTRY_NOT_FOUND`
/// - `PLAYLIST_WRITE_RATE_LIMITED` — more than 10 writes in 60 seconds;
///   `retry_after` says how long to wait.
/// - `SNAPSHOT_FAILED`, `WRITE_FAILED` — the playlist was left unchanged.
#[derive(Debug, Clone, Copy)]
pub struct Playlists<'a> {
    pub(super) v2: V2<'a>,
}

impl Playlists<'_> {
    fn write(self, method: Method, path: &str) -> reqwest::RequestBuilder {
        self.v2.req(method, path).timeout(WRITE_TIMEOUT)
    }

    /// Create a playlist and return its ID.
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn create(&self, playlist: &NewPlaylist) -> Result<String, CiderError> {
        let data: IdData = fetch(self.write(Method::POST, "/playlists").json(playlist)).await?;
        Ok(data.id)
    }

    /// Change a playlist's name, description or visibility.
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn update(&self, id: &str, update: &PlaylistUpdate) -> Result<(), CiderError> {
        let path = format!("/playlists/{}", encode(id));
        let _: IdData = fetch(self.write(Method::PATCH, &path).json(update)).await?;
        Ok(())
    }

    /// Append 1–100 tracks to a playlist.
    ///
    /// Catalog tracks are also added to the user's library, so they are only
    /// accepted when `allow_library_import` is `true`.
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn add_tracks(
        &self,
        id: &str,
        tracks: &[TrackRef],
        allow_library_import: bool,
    ) -> Result<PlaylistWrite, CiderError> {
        let path = format!("/playlists/{}/tracks", encode(id));
        fetch(self.write(Method::POST, &path).json(&AddTracksBody {
            tracks,
            allow_library_import,
        }))
        .await
    }

    /// Remove tracks from a playlist by ID and/or position.
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn remove_tracks(
        &self,
        id: &str,
        selection: &TrackSelection,
    ) -> Result<PlaylistWrite, CiderError> {
        let path = format!("/playlists/{}/tracks", encode(id));
        fetch(self.write(Method::DELETE, &path).json(selection)).await
    }

    /// Replace a playlist's whole track list (at most 500 tracks; an empty
    /// list clears it).
    ///
    /// `expected_track_count` must equal the playlist's current length. It
    /// guards against overwriting a playlist based on a stale copy.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with code `TRACK_COUNT_MISMATCH` if
    /// the playlist does not have `expected_track_count` tracks. See also
    /// the [type-level docs](Self#errors).
    pub async fn replace_tracks(
        &self,
        id: &str,
        tracks: &[TrackRef],
        expected_track_count: u32,
        allow_library_import: bool,
    ) -> Result<PlaylistWrite, CiderError> {
        let path = format!("/playlists/{}/tracks", encode(id));
        fetch(self.write(Method::PUT, &path).json(&ReplaceTracksBody {
            tracks,
            expected_track_count,
            allow_library_import,
        }))
        .await
    }

    /// Move the track at 0-based position `from` to position `to`.
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn move_track(
        &self,
        id: &str,
        from: u32,
        to: u32,
    ) -> Result<PlaylistWrite, CiderError> {
        let path = format!("/playlists/{}/tracks/move", encode(id));
        fetch(self.write(Method::POST, &path).json(&MoveBody { from, to })).await
    }

    /// List the saved snapshots of a playlist, for [`restore`](Self::restore).
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn history(&self, id: &str) -> Result<Vec<PlaylistHistoryEntry>, CiderError> {
        let path = format!("/playlists/{}/history", encode(id));
        let data: HistoryData =
            fetch(self.v2.req(Method::GET, &path).timeout(REMOTE_TIMEOUT)).await?;
        Ok(data.entries)
    }

    /// Restore a playlist to a snapshot. The current state is snapshotted
    /// first, so a restore can itself be undone.
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn restore(&self, id: &str, timestamp: &str) -> Result<PlaylistWrite, CiderError> {
        let path = format!("/playlists/{}/history/restore", encode(id));
        fetch(
            self.write(Method::POST, &path)
                .json(&TimestampBody { timestamp }),
        )
        .await
    }
}
