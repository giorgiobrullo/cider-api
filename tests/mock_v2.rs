mod common;

use cider_api::v2::*;
use cider_api::CiderError;
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn ok(data: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({ "data": data }))
}

fn err(status: u16, code: &str, message: &str) -> ResponseTemplate {
    ResponseTemplate::new(status)
        .set_body_json(json!({ "error": { "code": code, "message": message } }))
}

// ── Version detection ──

#[tokio::test]
async fn server_info_returns_info_on_cider_4() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/client/info"))
        .respond_with(ok(json!({
            "framework": "electron", "platform": "darwin", "osVersion": "27.2.0",
            "port": 10767, "production": true, "version": "4.0.25"
        })))
        .mount(&server)
        .await;

    let info = client.server_info().await.unwrap().unwrap();
    assert_eq!(info.version.as_deref(), Some("4.0.25"));
    assert_eq!(info.port, Some(10767));
    assert!(info.production);
}

#[tokio::test]
async fn server_info_is_none_on_legacy_cider() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/client/info"))
        .respond_with(ResponseTemplate::new(404).set_body_string("File not found"))
        .mount(&server)
        .await;

    assert!(client.server_info().await.unwrap().is_none());
}

#[tokio::test]
async fn v2_call_on_legacy_cider_is_unsupported() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/playback"))
        .respond_with(ResponseTemplate::new(404).set_body_string("File not found"))
        .mount(&server)
        .await;

    let e = client.v2().playback().snapshot().await.unwrap_err();
    assert!(matches!(e, CiderError::Unsupported));
}

// ── Errors ──

#[tokio::test]
async fn bad_token_is_unauthorized() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/client/info"))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(json!({ "error": "UNAUTHORIZED_APP_TOKEN" })),
        )
        .mount(&server)
        .await;

    let e = client.server_info().await.unwrap_err();
    assert!(matches!(e, CiderError::Unauthorized));
}

#[tokio::test]
async fn missing_scope_is_structured_error() {
    let (server, client) = common::setup_with_token("tok").await;
    Mock::given(method("GET"))
        .and(path("/api/v2/audio/volume"))
        .and(header("apptoken", "tok"))
        .respond_with(err(403, "INSUFFICIENT_SCOPE", "Required scope: audio"))
        .mount(&server)
        .await;

    match client.v2().audio().volume().await.unwrap_err() {
        CiderError::Server {
            status,
            code,
            message,
            retry_after,
        } => {
            assert!(retry_after.is_none());
            assert_eq!(status, 403);
            assert_eq!(code, "INSUFFICIENT_SCOPE");
            assert_eq!(message, "Required scope: audio");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn structured_404_is_not_mistaken_for_unsupported() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/playback/play"))
        .respond_with(err(404, "NO_CURRENT_TRACK", "Nothing is currently playing"))
        .mount(&server)
        .await;

    let e = client.v2().playback().play().await.unwrap_err();
    assert!(matches!(e, CiderError::Server { status: 404, .. }));
}

// ── Playback ──

#[tokio::test]
async fn snapshot_parses_full_state() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/playback"))
        .respond_with(ok(json!({
            "state": "playing",
            "nowPlaying": {
                "name": "Never Be Like You", "artistName": "Flume", "albumName": "Skin",
                "durationInMillis": 234_000, "inFavorites": true, "inLibrary": true, "flavor": "256"
            },
            "time": { "currentTime": 42.5, "duration": 234.0, "remaining": 191.5 },
            "volume": 0.65, "repeatMode": "all", "shuffleMode": true, "autoplay": false, "rate": 1
        })))
        .mount(&server)
        .await;

    let s = client.v2().playback().snapshot().await.unwrap();
    assert_eq!(s.state, PlaybackState::Playing);
    assert_eq!(s.repeat_mode, RepeatMode::All);
    assert!(s.shuffle_mode);
    assert!((s.time.current_time - 42.5).abs() < f64::EPSILON);
    let np = s.now_playing.unwrap();
    assert_eq!(np.name, "Never Be Like You");
    assert!(np.in_library);
}

#[tokio::test]
async fn now_playing_is_none_when_nothing_loaded() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/playback/now-playing"))
        .respond_with(ok(json!({ "currentPlaybackTime": 0, "inLibrary": false })))
        .mount(&server)
        .await;

    assert!(client
        .v2()
        .playback()
        .now_playing()
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn commands_send_expected_requests() {
    let (server, client) = common::setup().await;
    for (m, p, body) in [
        (
            "POST",
            "/api/v2/playback/previous",
            json!({ "force": true }),
        ),
        ("POST", "/api/v2/playback/seek", json!({ "position": 30.0 })),
        (
            "POST",
            "/api/v2/playback/play-item",
            json!({ "type": "songs", "id": "1" }),
        ),
        (
            "POST",
            "/api/v2/playback/play-collection",
            json!({ "type": "albums", "id": "2", "shuffle": true }),
        ),
        ("PUT", "/api/v2/playback/repeat", json!({ "mode": "none" })),
        (
            "PUT",
            "/api/v2/playback/shuffle",
            json!({ "enabled": true }),
        ),
        ("PATCH", "/api/v2/playback/rate", json!({ "rate": 4.0 })),
        ("PATCH", "/api/v2/audio/volume", json!({ "volume": 1.0 })),
        ("POST", "/api/v2/queue/move", json!({ "from": 3, "to": 1 })),
        (
            "PUT",
            "/api/v2/library/now-playing/rating",
            json!({ "rating": 1 }),
        ),
    ] {
        Mock::given(method(m))
            .and(path(p))
            .and(body_json(&body))
            .respond_with(ok(body.clone()))
            .expect(1)
            .mount(&server)
            .await;
    }

    let v2 = client.v2();
    v2.playback().previous(true).await.unwrap();
    v2.playback().seek(30.0).await.unwrap();
    v2.playback().play_item("songs", "1").await.unwrap();
    v2.playback()
        .play_collection("albums", "2", Some(true))
        .await
        .unwrap();
    v2.playback().set_repeat(RepeatMode::Off).await.unwrap();
    v2.playback().set_shuffle(true).await.unwrap();
    v2.playback().set_rate(9.0).await.unwrap(); // clamped to 4.0
    v2.audio().set_volume(1.5).await.unwrap(); // clamped to 1.0
    v2.queue().move_item(3, 1).await.unwrap();
    v2.library().set_rating(5).await.unwrap(); // clamped to 1
}

#[tokio::test]
async fn bodyless_commands_hit_expected_routes() {
    let (server, client) = common::setup().await;
    for (m, p) in [
        ("POST", "/api/v2/playback/play"),
        ("POST", "/api/v2/playback/toggle"),
        ("DELETE", "/api/v2/queue"),
        ("DELETE", "/api/v2/queue/items/4"),
        ("POST", "/api/v2/library/now-playing/love"),
        ("DELETE", "/api/v2/library/now-playing"),
    ] {
        Mock::given(method(m))
            .and(path(p))
            .respond_with(ok(json!({})))
            .expect(1)
            .mount(&server)
            .await;
    }

    let v2 = client.v2();
    v2.playback().play().await.unwrap();
    v2.playback().toggle().await.unwrap();
    v2.queue().clear().await.unwrap();
    v2.queue().remove(4).await.unwrap();
    v2.library().love().await.unwrap();
    v2.library().remove_now_playing().await.unwrap();
}

#[tokio::test]
async fn getters_unwrap_data() {
    let (server, client) = common::setup().await;
    for (p, data) in [
        ("/api/v2/playback/state", json!({ "state": "paused" })),
        ("/api/v2/playback/repeat", json!({ "mode": "one" })),
        ("/api/v2/playback/shuffle", json!({ "enabled": true })),
        ("/api/v2/playback/rate", json!({ "rate": 1.5 })),
        ("/api/v2/audio/volume", json!({ "volume": 0.25 })),
        (
            "/api/v2/library/now-playing/status",
            json!({ "inLibrary": true, "rating": -1 }),
        ),
        (
            "/api/v2/queue/position",
            json!({ "position": 2, "total": 9 }),
        ),
    ] {
        Mock::given(method("GET"))
            .and(path(p))
            .respond_with(ok(data))
            .mount(&server)
            .await;
    }

    let v2 = client.v2();
    assert_eq!(v2.playback().state().await.unwrap(), PlaybackState::Paused);
    assert_eq!(v2.playback().repeat().await.unwrap(), RepeatMode::One);
    assert!(v2.playback().shuffle().await.unwrap());
    assert!((v2.playback().rate().await.unwrap() - 1.5).abs() < f64::EPSILON);
    assert!((v2.audio().volume().await.unwrap() - 0.25).abs() < f32::EPSILON);
    let status = v2.library().now_playing_status().await.unwrap();
    assert!(status.in_library);
    assert_eq!(status.rating, -1);
    let pos = v2.queue().position().await.unwrap();
    assert_eq!((pos.position, pos.total), (2, 9));
}

// ── Queue ──

#[tokio::test]
async fn queue_list_merges_pagination_meta() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/queue"))
        .and(query_param("offset", "10"))
        .and(query_param("limit", "200"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "items": [{
                    "track": { "id": "123", "type": "song", "attributes": { "name": "Song" } },
                    "source": null,
                    "containerContext": { "id": "pl.1" }
                }],
                "position": 11
            },
            "meta": { "offset": 10, "limit": 200, "total": 42 }
        })))
        .mount(&server)
        .await;

    // limit above the server maximum is clamped to 200
    let page = client.v2().queue().list(10, 999).await.unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].track.id.as_deref(), Some("123"));
    assert!(page.items[0].container_context.is_some());
    assert_eq!(
        (page.position, page.offset, page.limit, page.total),
        (11, 10, 200, 42)
    );
}

// ── Auth ──

#[tokio::test]
async fn request_auth_returns_grant() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/request"))
        .and(body_json(
            json!({ "app_name": "My App", "scopes": ["playback", "queue"] }),
        ))
        .respond_with(ok(json!({ "token": "abc", "scopes": ["playback"] })))
        .mount(&server)
        .await;

    let grant = client
        .v2()
        .request_auth("My App", &[Scope::Playback, Scope::Queue])
        .await
        .unwrap();
    assert_eq!(grant.token, "abc");
    assert_eq!(grant.scopes, ["playback"]);
}

#[tokio::test]
async fn request_auth_denied_is_structured_error() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/request"))
        .respond_with(err(403, "AUTH_REQUEST_DENIED", "User denied the request."))
        .mount(&server)
        .await;

    let e = client
        .v2()
        .request_auth("My App", &[Scope::Playback])
        .await
        .unwrap_err();
    assert!(matches!(e, CiderError::Server { ref code, .. } if code == "AUTH_REQUEST_DENIED"));
}

// ── Rate limits ──

#[tokio::test]
async fn retry_after_header_is_exposed() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/playlists"))
        .respond_with(
            err(
                429,
                "PLAYLIST_WRITE_RATE_LIMITED",
                "Too many playlist writes",
            )
            .insert_header("Retry-After", "42"),
        )
        .mount(&server)
        .await;

    let e = client
        .v2()
        .playlists()
        .create(&NewPlaylist::named("x"))
        .await
        .unwrap_err();
    assert_eq!(e.code(), Some("PLAYLIST_WRITE_RATE_LIMITED"));
    match e {
        CiderError::Server {
            status,
            retry_after,
            ..
        } => {
            assert_eq!(status, 429);
            assert_eq!(retry_after, Some(std::time::Duration::from_secs(42)));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// ── Audio settings ──

#[tokio::test]
async fn audio_settings_round_trip() {
    let (server, client) = common::setup().await;
    let crossfade = json!({
        "enabled": true, "effectiveEnabled": false, "durationSec": 8, "skipEnabled": false,
        "tempoSensitivity": 0.6, "loudnessSensitivity": 0.75,
        "exclusions": { "albums": true, "playlists": false, "djMixes": true, "liveAlbums": true }
    });
    Mock::given(method("PATCH"))
        .and(path("/api/v2/audio/crossfade"))
        .and(body_json(
            json!({ "durationSec": 8.0, "exclusions": { "albums": true } }),
        ))
        .respond_with(ok(crossfade))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v2/audio/listening-mode"))
        .and(body_json(json!({ "mode": "antifatigue" })))
        .respond_with(ok(json!({ "mode": "antifatigue", "available": true })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v2/audio/radio-mode"))
        .and(body_json(
            json!({ "mode": "hdclassical", "bandwidthLimit": false }),
        ))
        .respond_with(ok(json!({
            "mode": "hdclassical", "amplify": 0, "processing": 100, "stereoBlend": 100,
            "bandwidthLimit": false, "available": true
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/audio/atmos"))
        .respond_with(ok(json!({ "enabled": true, "binaural": false, "devicePreferences": { "dev1": "passthrough" } })))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v2/audio/capture"))
        .respond_with(err(
            409,
            "CAPABILITY_UNAVAILABLE",
            "Host audio capture is not supported on this backend",
        ))
        .mount(&server)
        .await;

    let audio = client.v2().audio();
    let state = audio
        .set_crossfade(&CrossfadeUpdate {
            duration_sec: Some(8.0),
            exclusions: Some(TransitionExclusionsUpdate {
                albums: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(state.enabled && !state.effective_enabled && state.exclusions.albums);

    let mode = audio
        .set_listening_mode(ListeningMode::Antifatigue)
        .await
        .unwrap();
    assert_eq!(mode.mode, ListeningMode::Antifatigue);

    let radio = audio
        .set_radio_mode(&RadioUpdate {
            mode: Some(RadioMode::HdClassical),
            bandwidth_limit: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(radio.mode, RadioMode::HdClassical);

    let atmos = audio.atmos().await.unwrap();
    assert_eq!(
        atmos.device_preferences["dev1"],
        AtmosDeviceMode::Passthrough
    );

    let e = audio
        .set_capture(&CaptureUpdate {
            gain_db: Some(1.0),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert_eq!(e.code(), Some("CAPABILITY_UNAVAILABLE"));
}

// ── Library browsing ──

#[tokio::test]
async fn library_listing_pages_and_encodes_ids() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/library/playlists"))
        .and(query_param("offset", "0"))
        .and(query_param("limit", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "items": [
                { "id": "p.abc", "name": "Alternative", "description": "", "canEdit": false, "trackCount": null,
                  "artwork": { "url": "https://example.com/a.jpg", "width": 1200, "height": 1200 } },
                { "id": "p.def", "name": "Mine", "canEdit": true, "trackCount": 3, "artwork": null }
            ] },
            "meta": { "offset": 0, "limit": 100, "total": 22 }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/library/playlists/p.abc/songs"))
        .and(query_param("offset", "5"))
        .and(query_param("limit", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "items": [{ "id": "i.1", "name": "Excuse Me", "artistName": "Nothing But Thieves",
                                   "albumName": "NBT", "durationMs": 218_083, "trackNumber": 1, "artwork": null }] },
            "meta": { "offset": 5, "limit": 2, "total": 6 }
        })))
        .mount(&server)
        .await;

    let library = client.v2().library();
    let page = library.playlists(0, 1000).await.unwrap(); // clamped to 100
    assert_eq!(page.total, 22);
    assert_eq!(page.next_offset(), Some(2));
    assert!(!page.items[0].can_edit && page.items[0].track_count.is_none());
    assert_eq!(page.items[1].track_count, Some(3));
    assert!(page.items[1].artwork.is_none());

    let songs = library.playlist_songs("p.abc", 5, 2).await.unwrap();
    assert_eq!(songs.items[0].duration_ms, Some(218_083));
    assert_eq!(songs.next_offset(), None);
}

// ── Playlists ──

#[tokio::test]
async fn playlist_writes_send_expected_bodies() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/playlists"))
        .and(body_json(
            json!({ "name": "Road trip", "tracks": [{ "id": "i.1", "type": "library-songs" }] }),
        ))
        .respond_with(ok(json!({ "id": "p.new" })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/playlists/p.new/tracks"))
        .and(body_json(
            json!({ "tracks": [{ "id": "123", "type": "songs" }], "allowLibraryImport": true }),
        ))
        .respond_with(ok(
            json!({ "added": 1, "trackCount": 2, "snapshotAt": "2026-11-11T10:00:00.000Z" }),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/playlists/p.new/tracks"))
        .and(body_json(json!({ "indices": [0] })))
        .respond_with(ok(
            json!({ "removed": 1, "trackCount": 1, "snapshotAt": "t2" }),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/playlists/p.new/tracks"))
        .and(body_json(json!({ "tracks": [], "expectedTrackCount": 1 })))
        .respond_with(err(409, "TRACK_COUNT_MISMATCH", "expected 1, found 2"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/playlists/p.new/tracks/move"))
        .and(body_json(json!({ "from": 1, "to": 0 })))
        .respond_with(ok(json!({ "trackCount": 2, "snapshotAt": "t3" })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v2/playlists/p.new"))
        .and(body_json(json!({ "isPublic": true })))
        .respond_with(ok(json!({ "id": "p.new" })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/playlists/p.new/history"))
        .respond_with(ok(json!({ "entries": [{ "timestamp": "t2", "trackCount": 1, "tracks": [
            { "id": "i.1", "name": "Song", "artistName": "A", "albumName": "B", "durationInMillis": 1000, "artwork": "https://example.com/x.jpg" }
        ] }] })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/playlists/p.new/history/restore"))
        .and(body_json(json!({ "timestamp": "t2" })))
        .respond_with(ok(json!({ "trackCount": 1, "snapshotAt": "t4" })))
        .expect(1)
        .mount(&server)
        .await;

    let playlists = client.v2().playlists();
    let new = NewPlaylist {
        tracks: Some(vec![TrackRef::library_song("i.1")]),
        ..NewPlaylist::named("Road trip")
    };
    let id = playlists.create(&new).await.unwrap();
    assert_eq!(id, "p.new");

    let added = playlists
        .add_tracks(&id, &[TrackRef::song("123")], true)
        .await
        .unwrap();
    assert_eq!((added.added, added.track_count), (Some(1), 2));
    assert_eq!(
        added.snapshot_at.as_deref(),
        Some("2026-11-11T10:00:00.000Z")
    );

    let removed = playlists
        .remove_tracks(
            &id,
            &TrackSelection {
                indices: Some(vec![0]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(removed.removed, Some(1));

    let e = playlists
        .replace_tracks(&id, &[], 1, false)
        .await
        .unwrap_err();
    assert_eq!(e.code(), Some("TRACK_COUNT_MISMATCH"));

    assert_eq!(
        playlists.move_track(&id, 1, 0).await.unwrap().track_count,
        2
    );
    playlists
        .update(
            &id,
            &PlaylistUpdate {
                is_public: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let history = playlists.history(&id).await.unwrap();
    assert_eq!(history[0].tracks[0].name, "Song");
    assert_eq!(
        playlists
            .restore(&id, &history[0].timestamp)
            .await
            .unwrap()
            .snapshot_at
            .as_deref(),
        Some("t4")
    );
}

// ── Lyrics ──

#[tokio::test]
async fn lyrics_parse_and_send_options() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/lyrics/1765105174"))
        .and(query_param("words", "true"))
        .and(query_param("ttml", "false"))
        .and(query_param("provider", "apple-music"))
        .respond_with(ok(json!({
            "id": "1765105174",
            "source": { "provider": "Apple Music", "timingType": "Word", "language": "en-US" },
            "writers": ["Jordan Fish"],
            "lines": [
                { "start": 0, "end": 20.9, "text": "", "empty": true },
                { "start": 21.0, "end": 23.5, "text": "I was fine", "empty": false, "songPart": "Verse",
                  "isDuet": false, "isBackground": false,
                  "words": [{ "start": 21.0, "end": 21.3, "text": "I" }] }
            ]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/lyrics/current"))
        .respond_with(err(404, "NO_CURRENT_TRACK", "Nothing is currently playing"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/lyrics/1/translations"))
        .and(query_param("language", "it"))
        .respond_with(ok(json!({
            "language": "it",
            "credit": { "source": "x", "attributed": true, "creditText": "by x" },
            "translations": [{ "line": "hello", "translation": "ciao" }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/lyrics/1/variations"))
        .respond_with(ok(json!({ "trackId": "1", "variations": [{
            "variationId": 7, "contributorName": null, "language": "en", "syncType": "Line",
            "accessCount": 3, "trackName": "t", "artistName": "a"
        }] })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/lyrics/languages"))
        .respond_with(ok(
            json!({ "languages": [{ "value": "af", "label": "Afrikaans" }] }),
        ))
        .mount(&server)
        .await;

    let lyrics = client.v2().lyrics();
    let options = LyricsOptions {
        provider: Some(LyricsProvider::AppleMusic),
        words: true,
        ttml: false,
    };
    let song = lyrics.get("1765105174", options).await.unwrap();
    assert_eq!(song.lines.len(), 2);
    assert!(song.lines[0].empty);
    assert_eq!(song.lines[1].song_part.as_deref(), Some("Verse"));
    assert_eq!(song.lines[1].words.as_ref().unwrap()[0].text, "I");

    let e = lyrics.current(LyricsOptions::default()).await.unwrap_err();
    assert_eq!(e.code(), Some("NO_CURRENT_TRACK"));

    let tr = lyrics.translations("1", "it").await.unwrap();
    assert_eq!(tr.translations[0].translation, "ciao");
    assert_eq!(
        lyrics.variations("1").await.unwrap().variations[0].variation_id,
        7
    );
    assert_eq!(lyrics.languages().await.unwrap()[0].value, "af");
}

// ── Catalog, config, tokens ──

#[tokio::test]
async fn catalog_builds_queries() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/amapi/search"))
        .and(query_param("term", "daft punk"))
        .and(query_param("types", "songs,albums"))
        .and(query_param("limit", "25"))
        .respond_with(ok(json!({ "results": { "songs": { "data": [] } } })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/amapi/catalog/albums/123"))
        .and(query_param("include", "tracks,artists"))
        .respond_with(ok(json!({ "data": [{ "id": "123" }] })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/amapi/raw/v1/me/library/playlists"))
        .and(query_param("limit", "1"))
        .respond_with(ok(json!({ "data": [] })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/amapi/catalog/songs/1"))
        .respond_with(err(404, "AMAPI_ERROR", "Apple Music request failed"))
        .mount(&server)
        .await;

    let catalog = client.v2().catalog();
    let options = SearchOptions {
        types: vec!["songs".into(), "albums".into()],
        limit: Some(99),
        ..Default::default()
    };
    assert!(catalog.search("daft punk", &options).await.unwrap()["results"].is_object());
    let options = CatalogOptions {
        include: vec!["tracks".into(), "artists".into()],
        ..Default::default()
    };
    assert_eq!(
        catalog.get("albums", "123", &options).await.unwrap()["data"][0]["id"],
        "123"
    );
    catalog
        .raw("/v1/me/library/playlists?limit=1")
        .await
        .unwrap();
    let e = catalog
        .get("songs", "1", &CatalogOptions::default())
        .await
        .unwrap_err();
    assert!(matches!(e, CiderError::Server { status: 404, ref code, .. } if code == "AMAPI_ERROR"));
}

#[tokio::test]
async fn config_and_tokens() {
    let (server, client) = common::setup().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/config"))
        .and(query_param("path", "audio.normalization"))
        .respond_with(ok(json!(true)))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/client/tokens"))
        .respond_with(ok(
            json!({ "developerToken": "dev", "userToken": "user", "storefront": "it" }),
        ))
        .mount(&server)
        .await;

    assert_eq!(
        client
            .v2()
            .config_path("audio.normalization")
            .await
            .unwrap(),
        json!(true)
    );
    let tokens = client.v2().client_tokens().await.unwrap();
    assert_eq!(
        (tokens.developer_token.as_str(), tokens.storefront.as_str()),
        ("dev", "it")
    );
}

#[tokio::test]
async fn request_auth_with_image_sends_image() {
    let (server, client) = common::setup().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/request"))
        .and(body_json(json!({ "app_name": "My App", "app_image": "https://example.com/i.png", "scopes": ["lyrics"] })))
        .respond_with(ok(json!({ "token": "abc", "scopes": ["lyrics"], "origins": [] })))
        .expect(1)
        .mount(&server)
        .await;
    let grant = client
        .v2()
        .request_auth_with_image("My App", "https://example.com/i.png", &[Scope::Lyrics])
        .await
        .unwrap();
    assert!(grant.origins.is_empty());
}

#[tokio::test]
async fn request_auth_waits_longer_than_the_default_timeout() {
    let (server, client) = common::setup().await;
    // The client's default request timeout is 2 s; the auth prompt can take minutes.
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/request"))
        .respond_with(
            ok(json!({ "token": "abc", "scopes": ["playback"] }))
                .set_delay(std::time::Duration::from_secs(3)),
        )
        .mount(&server)
        .await;

    let grant = client
        .v2()
        .request_auth("My App", &[Scope::Playback])
        .await
        .unwrap();
    assert_eq!(grant.token, "abc");
}
