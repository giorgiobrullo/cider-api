<p align="center">
  <img src="assets/logo.svg" alt="cider-api" width="420">
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-f74c00?style=for-the-badge&logo=rust&logoColor=white" alt="Rust"></a>
</p>

Async Rust client for the [Cider](https://cider.sh) music player REST API.

Cider exposes a local HTTP API (default port **10767**) for controlling playback, managing the queue, and querying track information. This crate provides a fully typed, async client built on [reqwest](https://docs.rs/reqwest).

## Quick start

```rust
use cider_api::CiderClient;

#[tokio::main]
async fn main() -> Result<(), cider_api::CiderError> {
    let client = CiderClient::new();

    // Check if Cider is running
    client.is_active().await?;

    // Get the currently playing track
    if let Some(track) = client.now_playing().await? {
        println!("{} — {}", track.name, track.artist_name);
        println!("Album: {}", track.album_name);
        println!("Artwork: {}", track.artwork_url(600));
        println!(
            "Position: {:.1}s / {:.1}s",
            track.current_playback_time,
            track.duration_in_millis as f64 / 1000.0,
        );

        if !track.audio_traits.is_empty() {
            println!("Audio: {}", track.audio_traits.join(", "));
        }
    }

    // Control playback
    client.pause().await?;
    client.seek_ms(30_000).await?;
    client.play().await?;

    Ok(())
}
```

## Authentication

If Cider has API authentication enabled, pass the token via `with_token`:

```rust
let client = CiderClient::new().with_token("your-api-token");
```

The token is sent in the `apitoken` header on every request (no `Bearer` prefix). Generate one in Cider under **Settings > Connectivity > Manage External Application Access**.

## API coverage

| Category | Methods |
|---|---|
| **Status** | `is_active`, `is_playing`, `now_playing` |
| **Playback** | `play`, `pause`, `play_pause`, `stop`, `next`, `previous`, `seek`, `seek_ms` |
| **Play items** | `play_url`, `play_item`, `play_item_href`, `play_next`, `play_later` |
| **Queue** | `get_queue`, `queue_move_to_position`, `queue_remove_by_index`, `clear_queue` |
| **Volume** | `get_volume`, `set_volume` |
| **Settings** | `get_repeat_mode`, `toggle_repeat`, `get_shuffle_mode`, `toggle_shuffle`, `get_autoplay`, `toggle_autoplay` |
| **Library** | `add_to_library`, `set_rating` |
| **Apple Music API** | `amapi_run_v3` |

## Response types

All response types are fully typed with serde and match the [Cider RPC documentation](https://cider.sh/docs/client/rpc).

### `NowPlaying`

Returned by `now_playing()`. Contains Apple Music catalog metadata plus live playback state.

| Field | Type | Description |
|---|---|---|
| `name` | `String` | Song name |
| `artist_name` | `String` | Artist name |
| `album_name` | `String` | Album name |
| `artwork` | `Artwork` | Artwork with URL template |
| `duration_in_millis` | `u64` | Total duration |
| `current_playback_time` | `f64` | Position in seconds |
| `remaining_time` | `f64` | Remaining time in seconds |
| `play_params` | `Option<PlayParams>` | Song ID and kind |
| `url` | `Option<String>` | Apple Music web URL |
| `isrc` | `Option<String>` | International Standard Recording Code |
| `genre_names` | `Vec<String>` | Genre list |
| `track_number` | `u32` | Track number on album |
| `disc_number` | `u32` | Disc number |
| `release_date` | `Option<String>` | ISO-8601 release date |
| `audio_locale` | `Option<String>` | Audio locale (e.g. `"en-US"`) |
| `composer_name` | `Option<String>` | Composer / songwriter |
| `has_lyrics` | `bool` | Has lyrics |
| `has_time_synced_lyrics` | `bool` | Has karaoke-style lyrics |
| `is_apple_digital_master` | `bool` | Apple Digital Master |
| `audio_traits` | `Vec<String>` | e.g. `["atmos", "lossless", "spatial"]` |
| `previews` | `Vec<Preview>` | Audio preview URLs |
| `in_favorites` | `bool` | In user's favorites |
| `in_library` | `bool` | In user's library |
| `shuffle_mode` | `u8` | `0` = off, `1` = on |
| `repeat_mode` | `u8` | `0` = off, `1` = one, `2` = all |

### `QueueItem`

Returned by `get_queue()`. Includes track attributes, streaming internals, and container info.

| Field | Type | Description |
|---|---|---|
| `id` | `Option<String>` | Catalog ID |
| `item_type` | `Option<String>` | e.g. `"song"` |
| `attributes` | `Option<QueueItemAttributes>` | Track metadata (same fields as `NowPlaying`) |
| `state` | `Option<QueueItemState>` | `current == Some(2)` = now playing |
| `container` | `Option<QueueContainer>` | Source playlist/station/album |
| `context` | `Option<QueueContext>` | Queue context metadata |
| `asset_url` | `Option<String>` | HLS streaming URL |
| `flavor` | `Option<String>` | Audio codec descriptor |
| `assets` | `Option<Vec<Value>>` | Available audio flavors |
| `key_urls` | `Option<KeyUrls>` | DRM key URLs |

### `Artwork`

| Field | Type | Description |
|---|---|---|
| `width` | `u32` | Width in pixels |
| `height` | `u32` | Height in pixels |
| `url` | `String` | URL template (`{w}`, `{h}` placeholders) |
| `text_color1`–`4` | `Option<String>` | Hex text colors (station artwork) |
| `bg_color` | `Option<String>` | Hex background color (station artwork) |
| `has_p3` | `Option<bool>` | Display P3 color space |

Use `artwork.url_for_size(300)` to get a resolved URL.

## Error handling

All async methods return `Result<_, CiderError>`. Match on variants to handle specific failures:

```rust
use cider_api::{CiderClient, CiderError};

async fn example() {
    let client = CiderClient::new();
    match client.is_active().await {
        Ok(()) => println!("Cider is running"),
        Err(CiderError::Unauthorized) => println!("Bad API token"),
        Err(CiderError::Http(e)) if e.is_connect() => println!("Cider not running"),
        Err(e) => println!("Other error: {e}"),
    }
}
```

| Variant | Meaning |
|---|---|
| `Http(reqwest::Error)` | Network or HTTP-level failure |
| `NotReachable` | Cider is not running or port unreachable |
| `Unauthorized` | API token was rejected (HTTP 401/403) |
| `NothingPlaying` | No track is loaded |
| `Api(String)` | Unexpected response from Cider |

## Prerequisites

- [Cider](https://cider.sh) running with an Apple Music subscription
- API enabled in Cider: **Settings > Connectivity > Manage External Application Access**
