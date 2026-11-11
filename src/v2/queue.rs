// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Queue endpoints of the v2 API.

use reqwest::Method;

use super::types::{EnabledData, Envelope, IndexBody, ItemBody, MoveBody, PageMeta, QueueData};
use super::{exec, fetch, send, QueuePage, QueuePosition, SmartQueueStatus, V2};
use crate::client::CiderError;

/// Largest page size the queue endpoint accepts.
const QUEUE_MAX_LIMIT: u32 = 200;

/// Queue endpoints. Created by [`V2::queue`].
#[derive(Debug, Clone, Copy)]
pub struct Queue<'a> {
    pub(super) v2: V2<'a>,
}

impl Queue<'_> {
    /// Get a page of the queue.
    ///
    /// `limit` is clamped to `1..=200`. The page also reports the index of
    /// the currently playing entry and the total queue length.
    ///
    /// Sends `GET /api/v2/queue`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn list(&self, offset: u32, limit: u32) -> Result<QueuePage, CiderError> {
        let limit = limit.clamp(1, QUEUE_MAX_LIMIT);
        let env: Envelope<QueueData> = send(
            self.v2
                .req(Method::GET, "/queue")
                .query(&[("offset", offset), ("limit", limit)]),
        )
        .await?;
        let meta = env.meta.unwrap_or(PageMeta {
            offset,
            limit,
            total: 0,
        });
        Ok(QueuePage {
            items: env.data.items,
            position: env.data.position,
            offset: meta.offset,
            limit: meta.limit,
            total: meta.total,
        })
    }

    /// Get the current position and queue length without fetching entries.
    ///
    /// Sends `GET /api/v2/queue/position`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn position(&self) -> Result<QueuePosition, CiderError> {
        fetch(self.v2.req(Method::GET, "/queue/position")).await
    }

    /// Jump to the entry at `index` and play it.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn jump(&self, index: u32) -> Result<(), CiderError> {
        exec(
            self.v2
                .req(Method::POST, "/queue/jump")
                .json(&IndexBody { index }),
        )
        .await
    }

    /// Add an item right after the current track.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn add_next(&self, item_type: &str, id: &str) -> Result<(), CiderError> {
        exec(
            self.v2
                .req(Method::POST, "/queue/add-next")
                .json(&ItemBody { item_type, id }),
        )
        .await
    }

    /// Add an item to the end of the queue.
    ///
    /// When autoplay has appended suggestions to the queue, the item goes
    /// after the tracks the user queued and before those suggestions — not
    /// at the very last index.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn add_later(&self, item_type: &str, id: &str) -> Result<(), CiderError> {
        exec(
            self.v2
                .req(Method::POST, "/queue/add-later")
                .json(&ItemBody { item_type, id }),
        )
        .await
    }

    /// Remove the entry at `index`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn remove(&self, index: u32) -> Result<(), CiderError> {
        exec(
            self.v2
                .req(Method::DELETE, &format!("/queue/items/{index}")),
        )
        .await
    }

    /// Move the entry at index `from` to index `to`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn move_item(&self, from: u32, to: u32) -> Result<(), CiderError> {
        exec(
            self.v2
                .req(Method::POST, "/queue/move")
                .json(&MoveBody { from, to }),
        )
        .await
    }

    /// Remove every entry from the queue.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn clear(&self) -> Result<(), CiderError> {
        exec(self.v2.req(Method::DELETE, "/queue")).await
    }

    /// Get the Smart Queue Optimizer status.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn smart_optimizer(&self) -> Result<SmartQueueStatus, CiderError> {
        fetch(self.v2.req(Method::GET, "/queue/smart-optimizer")).await
    }

    /// Toggle the Smart Queue Optimizer. Returns `true` if it is now on.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError`] if the request fails or the server rejects it.
    pub async fn toggle_smart_optimizer(&self) -> Result<bool, CiderError> {
        let data: EnabledData =
            fetch(self.v2.req(Method::POST, "/queue/smart-optimizer/toggle")).await?;
        Ok(data.enabled)
    }
}
