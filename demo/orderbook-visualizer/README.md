# Orderbook Visualizer with Time Travel

A real-time orderbook visualization component for Kraken's WebSocket API with time-travel capabilities.

## Features

- **Real-time Orderbook Updates**: Live bid/ask depth visualization
- **Time Travel**: Replay historical orderbook states
- **Interactive UI**: Zoom, pan, and explore orderbook dynamics
- **Heatmap Visualization**: See liquidity depth at different price levels
- **Snapshot & Replay**: Save and replay orderbook states
- **Performance Optimized**: Handles high-frequency updates

## Architecture

```
┌─────────────────┐
│ Kraken WS API   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐      ┌──────────────────┐
│  Backend (Rust) │◄────►│  Time Series DB  │
│  - WS Handler   │      │  - Snapshots     │
│  - State Mgmt   │      │  - Deltas        │
└────────┬────────┘      └──────────────────┘
         │
         ▼
┌─────────────────┐
│ Frontend (React)│
│ - Visualizer    │
│ - Time Controls │
│ - Heatmap       │
└─────────────────┘
```

## Quick Start

### Backend

```bash
cd orderbook-visualizer/backend
cargo run
```

Server starts on `http://localhost:3033`

### Frontend

```bash
cd orderbook-visualizer/frontend
npm install
npm start
```

UI available at `http://localhost:3000`

## Usage

### Basic Visualization

```javascript
import OrderbookVisualizer from './components/OrderbookVisualizer';

function App() {
  return (
    <OrderbookVisualizer
      symbol="BTC/USD"
      depth={20}
      autoUpdate={true}
    />
  );
}
```

### Time Travel

```javascript
<OrderbookVisualizer
  symbol="BTC/USD"
  timeTravel={true}
  startTime="2024-01-01T00:00:00Z"
  endTime="2024-01-01T23:59:59Z"
  playbackSpeed={2.0}
/>
```

## API Endpoints

### WebSocket
- `ws://localhost:3033/ws/orderbook/:symbol` - Real-time orderbook updates

### REST
- `GET /api/orderbook/:symbol` - Current orderbook state
- `GET /api/orderbook/:symbol/history?from=<ts>&to=<ts>` - Historical data
- `GET /api/orderbook/:symbol/snapshot/:timestamp` - Specific snapshot

## Component Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `symbol` | string | required | Trading pair (e.g., "BTC/USD") |
| `depth` | number | 20 | Number of levels to display |
| `autoUpdate` | boolean | true | Enable real-time updates |
| `timeTravel` | boolean | false | Enable time travel mode |
| `startTime` | string | null | Start timestamp for replay |
| `endTime` | string | null | End timestamp for replay |
| `playbackSpeed` | number | 1.0 | Replay speed multiplier |
| `theme` | string | 'dark' | Color theme ('dark' or 'light') |

## Examples

See the `examples/` directory for:
- Basic orderbook display
- Time travel replay
- Multi-symbol comparison
- Liquidity heatmap

## Development

### Backend Development

```bash
cd backend
cargo watch -x run
```

### Frontend Development

```bash
cd frontend
npm run dev
```

## License

MIT
