# ⚠️ DEMO ONLY - NOT FOR PRODUCTION

This folder contains demonstration applications for the Kraken WebSocket SDK.

**These demos are:**
- For learning and testing purposes only
- Not production-ready
- Not security audited
- Using public market data only (no auth keys required)

## Available Demos

### web_demo/
A web-based SDK observability console showing:
- Live market data from Kraken's public WebSocket
- Connection state visualization
- Latency tracking
- Sequence gap detection

```bash
cd web_demo && cargo run
# Open http://localhost:3032
```

### orderbook-visualizer/
A React + Rust orderbook visualization tool.

---

**For production use, see the main SDK documentation in the [root README](../README.md).**
