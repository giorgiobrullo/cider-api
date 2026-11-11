# Changelog

## [0.2.0] - Unreleased

Adds support for the v2 API introduced in Cider 4. The existing v1 methods are
unchanged and keep working on Cider 2, 3 and 4. Tested against Cider 4.0.40.

### Added

- `CiderClient::v2()` — client for Cider 4's `/api/v2`, grouped by area:
  - `playback()`: one-shot `snapshot`, `state`, `time`, `audio_quality`, transport controls, `play_collection`, playback rate, and explicit `set_repeat` / `set_shuffle` / `set_autoplay` setters.
  - `queue()`: paginated `list`, `position`, `jump`, `add_next`, `add_later`, `remove`, `move_item`, `clear`, Smart Queue Optimizer.
  - `audio()`: volume, crossfade, automix, Dolby Atmos, Listening Mode, Radio Sound, host audio capture and normalization, each with a getter and a partial-update setter.
  - `library()`: status, add/remove, love/dislike and rating for the current track; paginated playlists, albums, artists, songs and recently added.
  - `playlists()`: create, rename, add/remove/replace/move tracks, history and restore.
  - `catalog()`: Apple Music catalog search, lookups and a read-only raw passthrough.
  - `lyrics()`: lyrics for the current track or any song, community variations, translations.
  - `config()` / `config_path()`: read-only Cider settings.
  - `client_tokens()`: MusicKit tokens (redacted in `Debug` output).
  - `request_auth()` / `request_auth_with_image()`: ask the user for a scoped API token via Cider's approval prompt.
- `CiderClient::server_info()` — detects v2 support; returns `None` on older Cider.
- `CiderError::Server { status, code, message, retry_after }` for structured v2 errors, `CiderError::code()`, and `CiderError::Unsupported` for v2 calls against a Cider without v2.
- `CiderClient::queue_change_to_index()` (v1).
- CI: formatting, clippy, tests, docs and an MSRV check.

### Changed

- **Breaking:** `CiderError` is now `#[non_exhaustive]`; exhaustive `match`es need a wildcard arm.
- `CiderClient`'s `Debug` output no longer prints the API token.
- Minimum supported Rust version is now 1.85. The previous 1.75 claim no longer held: current versions of the dependencies require the 2024 edition.

### Fixed

- v1 queue indices (`queue_move_to_position`, `queue_remove_by_index`) were documented as 1-based; they are 0-based.
- Crate docs said the auth header was `apitoken`; it is `apptoken`.
- README linked to a Cider documentation page that no longer exists.

## [0.1.1] - 2026-02-19

### Fixed

- Auth header is `apptoken`, not `apitoken`. Requests with `with_token()` were silently rejected by Cider.

### Added

- `CiderClient::with_base_url()` (hidden) for pointing the client at arbitrary URLs (useful for testing).
- Comprehensive test suite: 42 unit tests, 55 mock integration tests (wiremock), 17 real-instance integration tests.

## [0.1.0] - 2026-02-19

Initial release.

[0.2.0]: https://github.com/giorgiobrullo/cider-api/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/giorgiobrullo/cider-api/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/giorgiobrullo/cider-api/releases/tag/v0.1.0
