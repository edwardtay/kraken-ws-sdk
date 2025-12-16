# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Correctness contract documentation (reconnection, gaps, stitching)
- Tuning guide with buffer size recommendations
- Feature flags: `public`, `private`, `orderbook-state`, `metrics`, `full`
- Minimal `prelude` and extended `prelude_full` modules
- MSRV policy (Rust 1.70+)
- Security documentation

## [0.1.1] - 2024-12-16

### Fixed
- Removed `native` feature flag (always default now)
- Added compile-time check for wasm mutual exclusion

### Changed
- Replaced unmaintained `dotenv` with `dotenvy`

## [0.1.0] - 2024-12-16

### Added
- Initial release
- WebSocket client with auto-reconnection
- Support for public channels: ticker, trade, book, ohlc, spread
- Callback-based event handling
- Order book state management
- Backpressure control with configurable drop policies
- Latency tracking with percentile calculations
- Sequence gap detection and resync
- Multi-exchange abstraction layer
- Retry policies with circuit breaker
- Middleware system for request/response interception
- Telemetry and metrics collection
- WASM support for browser environments
- Web demo dashboard

### Security
- API credentials never logged (redacted in traces)
- Secure WebSocket (wss://) by default

---

## Versioning Policy

This project follows [Semantic Versioning](https://semver.org/):

- **MAJOR** (1.0.0): Breaking API changes
- **MINOR** (0.x.0): New features, backward compatible
- **PATCH** (0.0.x): Bug fixes, backward compatible

### Pre-1.0 Policy

While version is `0.x.y`:
- Minor version bumps may include breaking changes
- Patch versions are always backward compatible
- Breaking changes are documented in CHANGELOG

### MSRV (Minimum Supported Rust Version)

- Current MSRV: **1.70**
- MSRV bumps are considered breaking changes
- MSRV is tested in CI on every PR
