#![allow(dead_code)]

pub fn now_playing_json() -> &'static str {
    r#"{
        "status": "ok",
        "info": {
            "name": "Never Be Like You",
            "artistName": "Flume",
            "albumName": "Skin",
            "artwork": {
                "width": 3000,
                "height": 3000,
                "url": "https://example.com/{w}x{h}bb.jpg"
            },
            "durationInMillis": 234000,
            "playParams": { "id": "1719861213", "kind": "song" },
            "url": "https://music.apple.com/ca/album/skin/1719860281",
            "isrc": "AUUM71600506",
            "currentPlaybackTime": 42.5,
            "remainingTime": 191.5,
            "shuffleMode": 1,
            "repeatMode": 0,
            "inFavorites": true,
            "inLibrary": true,
            "genreNames": ["Electronic", "Music"],
            "trackNumber": 3,
            "discNumber": 1,
            "releaseDate": "2016-05-27T12:00:00Z",
            "hasLyrics": true,
            "isAppleDigitalMaster": true,
            "audioTraits": ["lossless", "lossy-stereo"],
            "previews": [{"url": "https://audio-ssl.itunes.apple.com/preview.m4a"}]
        }
    }"#
}

pub fn is_playing_json(playing: bool) -> String {
    format!(r#"{{"status":"ok","is_playing":{playing}}}"#)
}

pub fn volume_json(vol: f32) -> String {
    format!(r#"{{"status":"ok","volume":{vol}}}"#)
}

pub fn repeat_mode_json(value: u8) -> String {
    format!(r#"{{"status":"ok","value":{value}}}"#)
}

pub fn shuffle_mode_json(value: u8) -> String {
    format!(r#"{{"status":"ok","value":{value}}}"#)
}

pub fn autoplay_json(value: bool) -> String {
    format!(r#"{{"status":"ok","value":{value}}}"#)
}

pub fn queue_json() -> &'static str {
    r#"[
        {
            "id": "1719861213",
            "type": "song",
            "attributes": {
                "name": "Never Be Like You",
                "artistName": "Flume",
                "albumName": "Skin",
                "durationInMillis": 234000
            },
            "_state": { "current": 2 }
        },
        {
            "id": "1719861214",
            "type": "song",
            "attributes": {
                "name": "Say It",
                "artistName": "Flume",
                "albumName": "Skin",
                "durationInMillis": 252000
            }
        }
    ]"#
}
