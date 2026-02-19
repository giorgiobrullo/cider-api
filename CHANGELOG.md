# Changelog

## [0.1.1] - 2026-02-19

### Fixed

- Auth header is `apptoken`, not `apitoken`. Requests with `with_token()` were silently rejected by Cider.

### Added

- `CiderClient::with_base_url()` (hidden) for pointing the client at arbitrary URLs (useful for testing).
- Comprehensive test suite: 42 unit tests, 55 mock integration tests (wiremock), 17 real-instance integration tests.

## [0.1.0] - 2026-02-19

Initial release.

[0.1.1]: https://github.com/giorgiobrullo/cider-api/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/giorgiobrullo/cider-api/releases/tag/v0.1.0
