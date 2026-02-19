// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! # Cider API
//!
//! Async Rust client for the [Cider](https://cider.sh) music player REST API.
//!
//! Cider exposes a local HTTP API (default port **10767**) for controlling
//! playback, managing the queue, and querying track information. This crate
//! provides a fully typed, async client built on [`reqwest`].
//!
//! ## Quick start
//!
//! ```no_run
//! use cider_api::CiderClient;
//!
//! # async fn example() -> Result<(), cider_api::CiderError> {
//! let client = CiderClient::new();
//!
//! // Check if Cider is running
//! client.is_active().await?;
//!
//! // Get the currently playing track
//! if let Some(track) = client.now_playing().await? {
//!     println!("{} — {}", track.name, track.artist_name);
//!     println!("Album: {}", track.album_name);
//!     println!("Artwork: {}", track.artwork_url(600));
//!     println!(
//!         "Position: {:.1}s / {:.1}s",
//!         track.current_playback_time,
//!         track.duration_in_millis as f64 / 1000.0,
//!     );
//! }
//!
//! // Control playback
//! client.pause().await?;
//! client.seek_ms(30_000).await?;
//! client.play().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Authentication
//!
//! If Cider has API authentication enabled (Settings > Connectivity > Manage
//! External Application Access), pass the token via [`CiderClient::with_token`]:
//!
//! ```no_run
//! # use cider_api::CiderClient;
//! let client = CiderClient::new().with_token("your-api-token");
//! ```
//!
//! The token is sent in the `apitoken` header — no `Bearer` prefix.
//!
//! ## API coverage
//!
//! | Category | Methods |
//! |---|---|
//! | **Status** | [`is_active`](CiderClient::is_active), [`is_playing`](CiderClient::is_playing), [`now_playing`](CiderClient::now_playing) |
//! | **Playback** | [`play`](CiderClient::play), [`pause`](CiderClient::pause), [`play_pause`](CiderClient::play_pause), [`stop`](CiderClient::stop), [`next`](CiderClient::next), [`previous`](CiderClient::previous), [`seek`](CiderClient::seek), [`seek_ms`](CiderClient::seek_ms) |
//! | **Play items** | [`play_url`](CiderClient::play_url), [`play_item`](CiderClient::play_item), [`play_item_href`](CiderClient::play_item_href), [`play_next`](CiderClient::play_next), [`play_later`](CiderClient::play_later) |
//! | **Queue** | [`get_queue`](CiderClient::get_queue), [`queue_move_to_position`](CiderClient::queue_move_to_position), [`queue_remove_by_index`](CiderClient::queue_remove_by_index), [`clear_queue`](CiderClient::clear_queue) |
//! | **Volume** | [`get_volume`](CiderClient::get_volume), [`set_volume`](CiderClient::set_volume) |
//! | **Settings** | [`get_repeat_mode`](CiderClient::get_repeat_mode), [`toggle_repeat`](CiderClient::toggle_repeat), [`get_shuffle_mode`](CiderClient::get_shuffle_mode), [`toggle_shuffle`](CiderClient::toggle_shuffle), [`get_autoplay`](CiderClient::get_autoplay), [`toggle_autoplay`](CiderClient::toggle_autoplay) |
//! | **Library** | [`add_to_library`](CiderClient::add_to_library), [`set_rating`](CiderClient::set_rating) |
//! | **Apple Music API** | [`amapi_run_v3`](CiderClient::amapi_run_v3) |

mod client;
mod types;

pub use client::{CiderClient, CiderError, DEFAULT_PORT};
pub use types::*;
