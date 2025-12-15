# Kraken WebSocket SDK - Web Demo

A beautiful, real-time web dashboard demonstrating the capabilities of the Kraken WebSocket SDK.

## 🌟 Features

- **Real-time Market Data**: Live cryptocurrency prices, trades, and order book updates
- **WebSocket Integration**: Demonstrates SDK's WebSocket connectivity and event handling
- **Responsive Design**: Works on desktop, tablet, and mobile devices
- **Interactive Dashboard**: Real-time price updates with visual animations
- **Connection Management**: Automatic reconnection with exponential backoff
- **API Endpoints**: RESTful API for market data access

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ with Cargo
- Modern web browser with WebSocket support

### Running the Demo

1. **Navigate to the demo directory:**
   ```bash
   cd examples/web_demo
   ```

2. **Install dependencies and run:**
   ```bash
   cargo run
   ```

3. **Open your browser:**
   ```
   http://localhost:3030
   ```

## 📊 Dashboard Features

### Market Data Cards
- **Real-time Prices**: Live price updates with color-coded changes
- **Bid/Ask Spreads**: Current market spreads and volumes
- **Trade Information**: Latest trade details with buy/sell indicators
- **Connection Status**: Real-time connection health monitoring

### Interactive Controls
- **🔄 Reconnect**: Manually reconnect WebSocket
- **🗑️ Clear Data**: Clear all market data from display
- **⏸️ Pause Updates**: Temporarily pause real-time updates

### API Endpoints
- **Market Data**: `GET /api/market-data` - Current market data in JSON
- **Health Check**: `GET /api/health` - Service health status
- **WebSocket**: `ws://localhost:3030/ws` - Real-time data stream

## 🏗️ Architecture

### Backend (Rust)
```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Kraken SDK    │───▶│   Web Server     │───▶│   WebSocket     │
│                 │    │   (Warp)         │    │   Clients       │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  Market Data    │    │   REST API       │    │   Real-time     │
│  Processing     │    │   Endpoints      │    │   Updates       │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

### Frontend (JavaScript)
```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   WebSocket     │───▶│   Market Data    │───▶│   UI Updates    │
│   Connection    │    │   Management     │    │   & Animation   │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  Auto Reconnect │    │   Data Storage   │    │   Responsive    │
│  & Error Handle │    │   & Formatting   │    │   Design        │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

## 🔧 Configuration

### Server Configuration
The demo server can be configured by modifying `src/main.rs`:

```rust
// Server settings
let port = 3030;
let host = [127, 0, 0, 1];

// Kraken WebSocket settings
let config = ClientConfig {
    endpoint: "wss://ws.kraken.com".to_string(),
    timeout: Duration::from_secs(30),
    buffer_size: 2048,
    ..Default::default()
};
```

### Monitored Symbols
Currently tracking these cryptocurrency pairs:
- **BTC/USD** - Bitcoin to US Dollar
- **ETH/USD** - Ethereum to US Dollar  
- **ADA/USD** - Cardano to US Dollar

Add more symbols by modifying the `symbols` array in `simulate_market_data()`.

## 📱 Mobile Support

The dashboard is fully responsive and optimized for mobile devices:
- **Touch-friendly controls**
- **Responsive grid layout**
- **Optimized for small screens**
- **Swipe gestures support**

## 🔌 WebSocket Protocol

### Client → Server Messages
```json
{
  "type": "heartbeat"
}
```

### Server → Client Messages
```json
{
  "symbol": "BTC/USD",
  "last_price": "45000.00",
  "bid": "44995.00",
  "ask": "45005.00",
  "volume": "123.45",
  "spread": "10.00",
  "last_trade": {
    "price": "45000.00",
    "volume": "0.5",
    "side": "Buy",
    "timestamp": "2023-12-15T10:30:00Z"
  },
  "connection_status": "Connected",
  "timestamp": "2023-12-15T10:30:00Z"
}
```

## 🎨 Customization

### Styling
Modify `static/index.html` CSS to customize:
- **Color scheme** - Change gradient backgrounds and accent colors
- **Layout** - Adjust grid layouts and card sizes
- **Animations** - Modify price change animations and transitions

### Data Sources
Switch from simulated to real data by:
1. Enabling real WebSocket connection in `src/main.rs`
2. Removing the `simulate_market_data()` call
3. Implementing proper Kraken API authentication if needed

## 🚨 Production Considerations

### Security
- **CORS Configuration**: Adjust CORS settings for production domains
- **Rate Limiting**: Implement rate limiting for API endpoints
- **Authentication**: Add authentication for private data access
- **HTTPS**: Use TLS/SSL in production environments

### Performance
- **Connection Pooling**: Implement WebSocket connection pooling
- **Data Compression**: Enable WebSocket compression
- **Caching**: Add Redis or similar for market data caching
- **Load Balancing**: Use multiple server instances for high traffic

### Monitoring
- **Health Checks**: Implement comprehensive health monitoring
- **Metrics**: Add Prometheus metrics for monitoring
- **Logging**: Structured logging with correlation IDs
- **Alerting**: Set up alerts for connection failures

## 🐛 Troubleshooting

### Common Issues

**WebSocket Connection Failed**
```
Error: Failed to connect to WebSocket
Solution: Check if server is running on port 3030
```

**No Market Data Updates**
```
Issue: Dashboard shows "Connecting..." indefinitely
Solution: Check browser console for WebSocket errors
```

**Port Already in Use**
```
Error: Address already in use (os error 98)
Solution: Kill existing process or change port in main.rs
```

### Debug Mode
Enable debug logging by setting environment variable:
```bash
RUST_LOG=debug cargo run
```

## 📚 Learning Resources

This demo showcases several key concepts:

1. **WebSocket Integration** - Real-time bidirectional communication
2. **Event-Driven Architecture** - Callback-based data processing
3. **Error Handling** - Graceful degradation and recovery
4. **Responsive Design** - Mobile-first web development
5. **API Design** - RESTful endpoints and WebSocket protocols

## 🤝 Contributing

To extend this demo:

1. **Add New Features** - Implement additional market data types
2. **Improve UI** - Enhance visual design and user experience
3. **Add Charts** - Integrate charting libraries for price history
4. **Performance** - Optimize for high-frequency data updates

## 📄 License

This demo is part of the Kraken WebSocket SDK and is licensed under the MIT License.

---

**Built with ❤️ using Rust, Warp, and vanilla JavaScript**