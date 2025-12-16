# ⚠️ DEMO ONLY

**This is a demonstration application, not production code.**

## Kraken WebSocket SDK - Observability Console

A web-based tool for visualizing SDK behavior with live Kraken market data.

### Features
- Live ticker data from `wss://ws.kraken.com` (public, no auth)
- Connection state machine visualization
- Latency tracking (p50, p95, p99)
- Sequence gap detection
- Fault injection for testing

### Running

```bash
cargo run
# Open http://localhost:3032
```

### What This Demo Shows
- How the SDK handles real WebSocket connections
- Reconnection behavior
- Message parsing
- Backpressure handling

### What This Demo Is NOT
- Production-ready code
- A trading interface
- Security audited
- Using any API keys or authentication

---

For production SDK usage, see the [main README](../../README.md).
