<p align="center">
  <img src="assets/logo.svg" alt="cider-api" width="420">
</p>

<p align="center">
  <a href="https://crates.io/crates/cider-api"><img src="https://img.shields.io/crates/v/cider-api?style=for-the-badge&color=f74c00" alt="crates.io"></a>
  <a href="https://docs.rs/cider-api"><img src="https://img.shields.io/docsrs/cider-api?style=for-the-badge" alt="docs.rs"></a>
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

The token is sent in the `apptoken` header on every request (no `Bearer` prefix). Generate one in Cider under **Settings > Connectivity > Manage External Application Access**.

## API coverage

| Category | Methods |
|---|---|
| **Status** | `is_active`, `is_playing`, `now_playing` |
| **Playback** | `play`, `pause`, `play_pause`, `stop`, `next`, `previous`, `seek`, `seek_ms` |
| **Play items** | `play_url`, `play_item`, `play_item_href`, `play_next`, `play_later` |
| **Queue** | `get_queue`, `queue_change_to_index`, `queue_move_to_position`, `queue_remove_by_index`, `clear_queue` |
| **Volume** | `get_volume`, `set_volume` |
| **Settings** | `get_repeat_mode`, `toggle_repeat`, `get_shuffle_mode`, `toggle_shuffle`, `get_autoplay`, `toggle_autoplay` |
| **Library** | `add_to_library`, `set_rating` |
| **Apple Music API** | `amapi_run_v3` |

## Cider versions and the v2 API

The methods above use Cider's v1 API, which every Cider version serves (2, 3 and 4). Cider 4 adds a much larger v2 API, available through `client.v2()`:

```rust
use cider_api::{v2::RepeatMode, CiderClient};

let client = CiderClient::new().with_token("your-api-token");

// `None` on Cider versions without the v2 API
if client.server_info().await?.is_some() {
    let v2 = client.v2();

    let snapshot = v2.playback().snapshot().await?;   // state, track, time, volume in one call
    v2.playback().set_repeat(RepeatMode::All).await?; // explicit setters, not just toggles

    let queue = v2.queue().list(0, 50).await?;        // paginated queue
    let playlists = v2.library().playlists(0, 100).await?;
    println!("{:?}, {} queued, {} playlists", snapshot.state, queue.total, playlists.total);
}
```

| Area | Scope | Methods |
|---|---|---|
| **`v2()`** | — | `client_info`, `request_auth`, `request_auth_with_image`, `client_tokens` (`account`), `config`, `config_path` (`config`) |
| **`v2().playback()`** | `playback` | `snapshot`, `now_playing`, `state`, `time`, `audio_quality`, `play`, `pause`, `stop`, `toggle`, `next`, `previous`, `seek`, `play_item`, `play_url`, `play_href`, `play_collection`, `rate`, `set_rate`, `repeat`, `set_repeat`, `toggle_repeat`, `shuffle`, `set_shuffle`, `toggle_shuffle`, `autoplay`, `set_autoplay`, `toggle_autoplay` |
| **`v2().queue()`** | `queue` | `list`, `position`, `jump`, `add_next`, `add_later`, `remove`, `move_item`, `clear`, `smart_optimizer`, `toggle_smart_optimizer` |
| **`v2().audio()`** | `audio` | `volume`, `crossfade`, `automix`, `atmos`, `listening_mode`, `radio_mode`, `capture`, `normalization`, and a `set_*` for each |
| **`v2().library()`** | `library` | `now_playing_status`, `add_now_playing`, `remove_now_playing`, `love`, `dislike`, `clear_rating`, `set_rating`, `playlists`, `playlist_songs`, `albums`, `album_songs`, `artists`, `artist_albums`, `songs`, `recently_added` |
| **`v2().playlists()`** | `library` | `create`, `update`, `add_tracks`, `remove_tracks`, `replace_tracks`, `move_track`, `history`, `restore` |
| **`v2().catalog()`** | `library` | `search`, `get`, `raw` (read-only Apple Music API proxy) |
| **`v2().lyrics()`** | `lyrics` | `current`, `get`, `variations`, `variation`, `translations`, `languages` |
| **`v2().plugins()`** | `plugins` | `install` |
| **`v2().events()`** | — | real-time event stream (cargo feature `events`) |

### Errors

On a Cider without v2, these methods return `CiderError::Unsupported`. API failures come back as `CiderError::Server { status, code, message, retry_after }`; `err.code()` gives Cider's machine-readable code:

```rust
match client.v2().audio().volume().await {
    Ok(volume) => println!("{volume}"),
    Err(e) if e.code() == Some("INSUFFICIENT_SCOPE") => println!("token lacks the audio scope"),
    Err(e) => println!("{e}"),
}
```

`CiderError::Interrupted` means Cider closed the connection before answering. Cider 4.0 does this to any request it has not answered within 30 seconds, which is shorter than its own consent prompts — see the note under scoped tokens.

### Events

With the `events` feature, `v2().events()` connects to Cider's Socket.IO channel and yields typed events. No token is needed.

```toml
cider-api = { version = "0.2", features = ["events"] }
```

```rust
use cider_api::v2::Event;

let mut events = client.v2().events().await?;
while let Some(event) = events.next().await {
    match event? {
        Event::NowPlayingItemChanged(change) => println!("now playing {}", change.track.name),
        Event::PlaybackStateChanged(change) => println!("{:?}", change.state),
        Event::VolumeChanged(volume) => println!("volume {volume}"),
        _ => {}
    }
}
```

Events cover the current track, playback state and position, transitions, volume/repeat/shuffle, every audio setting, the queue and playlist writes. Anything this crate has no type for arrives as `Event::Other { kind, data }`. Cider may deliver the same event twice. The stream ends when Cider closes the connection; call `events()` again to reconnect.

### Scoped tokens

v2 tokens can be limited to scopes (`playback`, `queue`, `library`, `audio`, `account`, `plugins`, `config`, `lyrics`). An app can ask the user for one — Cider shows an approval prompt:

```rust
use cider_api::v2::Scope;

let grant = CiderClient::new()
    .v2()
    .request_auth("My App", &[Scope::Playback, Scope::Queue])
    .await?;
let client = CiderClient::new().with_token(grant.token);
```

Ask the user to answer within 30 seconds. Cider 4.0 keeps the prompt open for two minutes but closes the request's connection after 30 seconds; a later answer ends in `CiderError::Interrupted` and the token is lost. The same applies to `plugins().install()`.

### Editing playlists

Cider snapshots a playlist before every edit and returns the restore point, so edits can be undone:

```rust
use cider_api::v2::TrackRef;

let playlists = client.v2().playlists();
let write = playlists.add_tracks("p.abc123", &[TrackRef::song("1719861213")], true).await?;
if let Some(snapshot) = write.snapshot_at {
    playlists.restore("p.abc123", &snapshot).await?; // undo
}
```

## Response types

All response types are fully typed with serde and match the [Cider RPC documentation](https://cider.gitbook.io/welcome-to-gitbook/docs/1.client/rpc).

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
