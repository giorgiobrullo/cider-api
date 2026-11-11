//! Integration tests against a live Cider instance.
//!
//! Skipped unless `CIDER_TEST_PORT` is set (e.g. `CIDER_TEST_PORT=10767`).
//! Optionally set `CIDER_TEST_TOKEN` for authenticated access.
//!
//! These tests run sequentially (`--test-threads=1` recommended) since they
//! share playback state on a single Cider instance.

use cider_api::CiderClient;

fn live_client() -> Option<CiderClient> {
    let port: u16 = std::env::var("CIDER_TEST_PORT").ok()?.parse().ok()?;
    let mut client = CiderClient::with_port(port);
    if let Ok(token) = std::env::var("CIDER_TEST_TOKEN") {
        client = client.with_token(token);
    }
    Some(client)
}

macro_rules! skip_unless_live {
    () => {
        match live_client() {
            Some(c) => c,
            None => {
                eprintln!("Skipping: CIDER_TEST_PORT not set");
                return;
            }
        }
    };
}

// ── Status ──

#[tokio::test]
async fn live_is_active() {
    let client = skip_unless_live!();
    client.is_active().await.unwrap();
}

#[tokio::test]
async fn live_is_playing() {
    let client = skip_unless_live!();
    let _ = client.is_playing().await.unwrap();
}

#[tokio::test]
async fn live_now_playing() {
    let client = skip_unless_live!();
    let result = client.now_playing().await.unwrap();
    if let Some(track) = result {
        assert!(!track.name.is_empty());
        eprintln!("Now playing: {} - {}", track.name, track.artist_name);

        // Verify helper methods work on real data
        let _ms = track.current_position_ms();
        let _url = track.artwork_url(300);
        let _id = track.song_id();
    }
}

// ── Playback control ──

#[tokio::test]
async fn live_play_pause_cycle() {
    let client = skip_unless_live!();

    // Save initial state
    let was_playing = client.is_playing().await.unwrap();

    // Pause
    client.pause().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(!client.is_playing().await.unwrap());

    // Play
    client.play().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(client.is_playing().await.unwrap());

    // play_pause toggle
    client.play_pause().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let after_toggle = client.is_playing().await.unwrap();
    // Toggle back to restore
    client.play_pause().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let after_second_toggle = client.is_playing().await.unwrap();
    assert_ne!(after_toggle, after_second_toggle);

    // Restore original state
    if was_playing {
        client.play().await.unwrap();
    } else {
        client.pause().await.unwrap();
    }
}

#[tokio::test]
async fn live_next_previous() {
    let client = skip_unless_live!();

    let before = client.now_playing().await.unwrap();
    client.next().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    client.previous().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Should be back to the same track (or close — not asserting equality
    // since queue behavior varies)
    let _ = client.now_playing().await.unwrap();
    let _ = before; // used for debugging if needed
}

#[tokio::test]
async fn live_seek() {
    let client = skip_unless_live!();

    // Seek to 10 seconds
    client.seek(10.0).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    if let Some(track) = client.now_playing().await.unwrap() {
        // Should be roughly around 10s (allow generous margin)
        let pos = track.current_playback_time;
        assert!(
            (9.0..=15.0).contains(&pos),
            "Position was {pos}s after seeking to 10s"
        );
    }

    // Also test seek_ms
    client.seek_ms(5000).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    if let Some(track) = client.now_playing().await.unwrap() {
        let pos = track.current_playback_time;
        assert!(
            (4.0..=10.0).contains(&pos),
            "Position was {pos}s after seeking to 5s"
        );
    }
}

// ── Volume ──

#[tokio::test]
async fn live_volume_set_and_restore() {
    let client = skip_unless_live!();

    let original = client.get_volume().await.unwrap();
    assert!((0.0..=1.0).contains(&original));

    // Set to a known value
    client.set_volume(0.42).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let after = client.get_volume().await.unwrap();
    assert!(
        (after - 0.42).abs() < 0.05,
        "Volume was {after} after setting to 0.42"
    );

    // Restore original
    client.set_volume(original).await.unwrap();
}

// ── Queue ──

#[tokio::test]
async fn live_get_queue() {
    let client = skip_unless_live!();
    let queue = client.get_queue().await.unwrap();
    eprintln!("Queue has {} items", queue.len());

    // If queue is non-empty, verify structure
    if !queue.is_empty() {
        // At least one item should be current if something is playing
        let current_count = queue.iter().filter(|i| i.is_current()).count();
        eprintln!("Current items: {current_count}");

        for (i, item) in queue.iter().enumerate() {
            if let Some(attrs) = &item.attributes {
                eprintln!(
                    "  [{i}] {} - {} {}",
                    attrs.name,
                    attrs.artist_name,
                    if item.is_current() { "(current)" } else { "" }
                );
            }
        }
    }
}

// ── Settings (toggle and restore) ──

#[tokio::test]
async fn live_repeat_mode_toggle_and_restore() {
    let client = skip_unless_live!();

    let original = client.get_repeat_mode().await.unwrap();
    assert!(original <= 2);

    // Toggle three times to cycle through all modes and back
    client.toggle_repeat().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let after_1 = client.get_repeat_mode().await.unwrap();

    client.toggle_repeat().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let after_2 = client.get_repeat_mode().await.unwrap();

    client.toggle_repeat().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let after_3 = client.get_repeat_mode().await.unwrap();

    // The three values should be distinct (0, 1, 2 in some order)
    eprintln!("Repeat cycle: {original} -> {after_1} -> {after_2} -> {after_3}");
    assert_eq!(
        after_3, original,
        "Three toggles should return to original mode"
    );
}

#[tokio::test]
async fn live_shuffle_mode_toggle_and_restore() {
    let client = skip_unless_live!();

    let original = client.get_shuffle_mode().await.unwrap();
    assert!(original <= 1);

    client.toggle_shuffle().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let toggled = client.get_shuffle_mode().await.unwrap();
    assert_ne!(original, toggled, "Shuffle should have changed");

    // Toggle back
    client.toggle_shuffle().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let restored = client.get_shuffle_mode().await.unwrap();
    assert_eq!(original, restored, "Shuffle should be back to original");
}

#[tokio::test]
async fn live_autoplay_toggle_and_restore() {
    let client = skip_unless_live!();

    let original = client.get_autoplay().await.unwrap();

    client.toggle_autoplay().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let toggled = client.get_autoplay().await.unwrap();
    assert_ne!(original, toggled, "Autoplay should have changed");

    // Toggle back
    client.toggle_autoplay().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let restored = client.get_autoplay().await.unwrap();
    assert_eq!(original, restored, "Autoplay should be back to original");
}

// ── Library / Rating ──

#[tokio::test]
async fn live_add_to_library() {
    let client = skip_unless_live!();
    // Only test if something is playing (no-op if already in library)
    if client.now_playing().await.unwrap().is_some() {
        client.add_to_library().await.unwrap();
    }
}

#[tokio::test]
async fn live_set_rating_and_clear() {
    let client = skip_unless_live!();
    if client.now_playing().await.unwrap().is_some() {
        // Like
        client.set_rating(1).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Clear rating
        client.set_rating(0).await.unwrap();
    }
}

// ── Token auth ──

#[tokio::test]
async fn live_valid_token_is_accepted() {
    let client = skip_unless_live!();
    client.is_active().await.unwrap();
}

#[tokio::test]
async fn live_bad_token_is_rejected() {
    let port: u16 = match std::env::var("CIDER_TEST_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
    {
        Some(p) => p,
        None => {
            eprintln!("Skipping: CIDER_TEST_PORT not set");
            return;
        }
    };

    // Only meaningful if auth is enabled
    if std::env::var("CIDER_TEST_TOKEN").is_err() {
        eprintln!("Skipping: CIDER_TEST_TOKEN not set, can't test auth rejection");
        return;
    }

    let bad_client = CiderClient::with_port(port).with_token("definitely-wrong-token");
    let result = bad_client.play().await;
    assert!(
        result.is_err(),
        "Expected error with bad token, got: {result:?}"
    );
}

// ── Apple Music API passthrough ──

#[tokio::test]
async fn live_amapi_run_v3() {
    let client = skip_unless_live!();

    // Search for something simple
    let result = client
        .amapi_run_v3("/v1/catalog/us/search?term=flume&types=songs&limit=1")
        .await
        .unwrap();

    // Should return valid JSON with results
    assert!(result.is_object(), "Expected JSON object, got: {result}");
    eprintln!(
        "AMAPI response keys: {:?}",
        result.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
}

// ── Stop ──

#[tokio::test]
async fn live_stop_and_resume() {
    let client = skip_unless_live!();

    let was_playing = client.is_playing().await.unwrap();
    if !was_playing {
        eprintln!("Skipping stop test: nothing playing");
        return;
    }

    client.stop().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // After stop, nothing should be playing
    assert!(!client.is_playing().await.unwrap());

    // Resume
    client.play().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
}

// ── v2 (Cider 4+) ──
//
// Read-only: these never change playback state. Skipped on older Cider.

#[tokio::test]
async fn live_v2_read_only() {
    let client = skip_unless_live!();
    let Some(info) = client.server_info().await.unwrap() else {
        eprintln!("Skipping: this Cider has no v2 API");
        return;
    };
    eprintln!("Cider {:?} on {:?}", info.version, info.platform);

    let v2 = client.v2();
    let snapshot = v2.playback().snapshot().await.unwrap();
    eprintln!(
        "snapshot: {:?} repeat={:?}",
        snapshot.state, snapshot.repeat_mode
    );
    let _ = v2.playback().state().await.unwrap();
    let _ = v2.playback().time().await.unwrap();
    let _ = v2.playback().now_playing().await.unwrap();
    let _ = v2.playback().audio_quality().await.unwrap();
    let _ = v2.playback().rate().await.unwrap();
    let _ = v2.playback().repeat().await.unwrap();
    let _ = v2.playback().shuffle().await.unwrap();
    let _ = v2.playback().autoplay().await.unwrap();
    let _ = v2.audio().volume().await.unwrap();
    let page = v2.queue().list(0, 5).await.unwrap();
    let pos = v2.queue().position().await.unwrap();
    assert_eq!(page.total, pos.total);
    let _ = v2.queue().smart_optimizer().await.unwrap();
    let _ = v2.library().now_playing_status().await.unwrap();
}

#[tokio::test]
async fn live_v2_audio_settings_read_only() {
    let client = skip_unless_live!();
    if client.server_info().await.unwrap().is_none() {
        return;
    }
    let audio = client.v2().audio();
    let _ = audio.crossfade().await.unwrap();
    let _ = audio.automix().await.unwrap();
    let _ = audio.atmos().await.unwrap();
    let _ = audio.listening_mode().await.unwrap();
    let _ = audio.capture().await.unwrap();
    let _ = audio.normalization().await.unwrap();
    // Added in Cider 4.0.40.
    match audio.radio_mode().await {
        Ok(_) | Err(cider_api::CiderError::Unsupported) => {}
        Err(e) => panic!("radio_mode: {e:?}"),
    }
}

#[tokio::test]
async fn live_v2_library_browsing() {
    let client = skip_unless_live!();
    if client.server_info().await.unwrap().is_none() {
        return;
    }
    let library = client.v2().library();
    let playlists = library.playlists(0, 5).await.unwrap();
    assert!(playlists.items.len() <= 5);
    if let Some(playlist) = playlists.items.first() {
        let _ = library.playlist_songs(&playlist.id, 0, 5).await.unwrap();
        let _ = client.v2().playlists().history(&playlist.id).await.unwrap();
    }
    let albums = library.albums(0, 5).await.unwrap();
    if let Some(album) = albums.items.first() {
        let _ = library.album_songs(&album.id, 0, 5).await.unwrap();
    }
    let artists = library.artists(0, 5).await.unwrap();
    if let Some(artist) = artists.items.first() {
        let _ = library.artist_albums(&artist.id, 0, 5).await.unwrap();
    }
    let _ = library.songs(0, 5).await.unwrap();
    let _ = library.recently_added(0, 5).await.unwrap();
}

#[tokio::test]
async fn live_v2_catalog_lyrics_config() {
    use cider_api::v2::{CatalogOptions, LyricsOptions, SearchOptions};

    let client = skip_unless_live!();
    if client.server_info().await.unwrap().is_none() {
        return;
    }
    let v2 = client.v2();

    let options = SearchOptions {
        types: vec!["songs".into()],
        limit: Some(1),
        ..Default::default()
    };
    let results = v2.catalog().search("daft punk", &options).await.unwrap();
    let song_id = results["results"]["songs"]["data"][0]["id"]
        .as_str()
        .expect("search returned a song")
        .to_string();
    let song = v2
        .catalog()
        .get("songs", &song_id, &CatalogOptions::default())
        .await
        .unwrap();
    assert_eq!(song["data"][0]["id"], song_id.as_str());
    let _ = v2
        .catalog()
        .raw("v1/me/library/songs?limit=1")
        .await
        .unwrap();
    let e = v2.catalog().raw("v2/not-allowed").await.unwrap_err();
    assert_eq!(e.code(), Some("INVALID_REQUEST"));

    assert!(!v2.lyrics().languages().await.unwrap().is_empty());
    match v2.lyrics().get(&song_id, LyricsOptions::default()).await {
        Ok(lyrics) => assert!(!lyrics.lines.is_empty()),
        Err(e) => assert_eq!(e.code(), Some("LYRICS_NOT_FOUND"), "{e:?}"),
    }

    assert!(v2.config().await.unwrap().is_object());
    let e = v2.config_path("definitely.not.there").await.unwrap_err();
    assert_eq!(e.code(), Some("CONFIG_PATH_NOT_FOUND"));
    let tokens = v2.client_tokens().await.unwrap();
    assert!(!tokens.storefront.is_empty());
}
