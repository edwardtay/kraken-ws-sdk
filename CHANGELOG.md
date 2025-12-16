# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **BREAKING:** Frozen public API surface with stability tiers (`prelude`, `extended`, `internal`)
- **BREAKING:** Deterministic connection state machine with explicit states
- `SdkEvent` unified event enum for stream-based consumption
- `client.events()` method returning `EventReceiver` stream
- `ConnectionState` enum with full state machine (DISCONNECTED, CONNECTING, AUTHENTICATING, SUBSCRIBING, SUBSCRIBED, RESYNCING, DEGRADED, CLOSED)
- `StateTransition` events emitted on every state change
- Correctness guarantees documentation (message ordering, heartbeat, timestamps)
- Tuning guide with buffer size recommendations
- Feature flags: `public`, `private`, `orderbook-state`, `metrics`, `full`
- MSRV policy (Rust 1.70+)
- Security documentation
- Compatibility promise and upgrade guide

### Changed
- **BREAKING:** `prelude` module now exports `KrakenClient` (alias for `KrakenWsClient`)
- **BREAKING:** Legacy exports marked `#[doc(hidden)]` (still work, but prefer `prelude`)
- Reorganized module structure for API stability

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

### Compatibility Promise

**For 1.x releases (post-1.0):**

| Change Type | Allowed in Minor/Patch? | Example |
|-------------|------------------------|---------|
| New `ClientConfig` fields | ✅ Yes (with defaults) | Adding `max_message_size` |
| New `Event` variants | ✅ Yes | Adding `Event::Heartbeat` |
| New methods on `KrakenClient` | ✅ Yes | Adding `client.ping()` |
| Removing public API | ❌ No | Removing `client.events()` |
| Changing method signatures | ❌ No | Changing return type |
| Changing behavior semantics | ❌ No | Changing reconnect logic |
| MSRV bump | ❌ No (major only) | Requiring Rust 1.75 |

**What counts as "breaking":**
- Removing or renaming public types/functions in `prelude`
- Changing function signatures (parameters, return types)
- Changing default behavior in ways that affect correctness
- Removing feature flags

**What is NOT breaking:**
- Adding new optional fields to config structs (with `Default`)
- Adding new enum variants (use `#[non_exhaustive]`)
- Adding new methods to existing types
- Performance improvements
- Bug fixes (even if they change incorrect behavior)

### Pre-1.0 Policy

While version is `0.x.y`:
- Minor version bumps MAY include breaking changes
- Patch versions are always backward compatible
- All breaking changes are documented with **BREAKING:** prefix
- Upgrade notes provided for breaking changes

### MSRV (Minimum Supported Rust Version)

- Current MSRV: **1.70**
- MSRV bumps are considered breaking changes (major version only post-1.0)
- MSRV is tested in CI on every PR
- We support the last 4 stable Rust releases

---

## Upgrade Guide

### Upgrading to 0.2.0 (when released)

**Breaking changes:**
- `prelude` module reorganized - use `use kraken_ws_sdk::prelude::*`
- `KrakenWsClient` renamed to `KrakenClient` in prelude
- `ConnectionState` now has additional states (AUTHENTICATING, RESYNCING, DEGRADED)

**Migration:**
```rust
// Before (0.1.x)
use kraken_ws_sdk::{KrakenWsClient, ClientConfig, ConnectionState};

// After (0.2.x)
use kraken_ws_sdk::prelude::*;
// KrakenClient is the new name in prelude
// Or use the legacy export:
use kraken_ws_sdk::KrakenWsClient;
```

### Upgrading from 0.1.0 to 0.1.1

No breaking changes. Safe to upgrade.
