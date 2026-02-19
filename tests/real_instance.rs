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
        assert!(pos >= 9.0 && pos <= 15.0, "Position was {pos}s after seeking to 10s");
    }

    // Also test seek_ms
    client.seek_ms(5000).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    if let Some(track) = client.now_playing().await.unwrap() {
        let pos = track.current_playback_time;
        assert!(pos >= 4.0 && pos <= 10.0, "Position was {pos}s after seeking to 5s");
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
                eprintln!("  [{i}] {} - {} {}", attrs.name, attrs.artist_name,
                    if item.is_current() { "(current)" } else { "" });
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
    assert_eq!(after_3, original, "Three toggles should return to original mode");
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
    let port: u16 = match std::env::var("CIDER_TEST_PORT").ok().and_then(|p| p.parse().ok()) {
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
    eprintln!("AMAPI response keys: {:?}", result.as_object().map(|o| o.keys().collect::<Vec<_>>()));
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
