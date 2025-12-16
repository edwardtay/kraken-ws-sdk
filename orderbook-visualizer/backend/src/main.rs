//! Orderbook Visualizer Backend Server

mod orderbook_manager;
mod storage;

use crate::orderbook_manager::{OrderbookManager, start_kraken_client};
use crate::storage::OrderbookSnapshot;
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use warp::Filter;

/// API query parameters for history endpoint
#[derive(Debug, Deserialize)]
struct HistoryQuery {
    from: Option<String>,
    to: Option<String>,
}

/// WebSocket message types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum WsMessage {
    #[serde(rename = "snapshot")]
    Snapshot { data: OrderbookSnapshot },
    #[serde(rename = "error")]
    Error { message: String },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("🚀 Starting Orderbook Visualizer Backend");

    // Create orderbook manager
    let manager = Arc::new(OrderbookManager::new("./data/orderbooks")?);

    // Default symbols to track
    let symbols = vec![
        "BTC/USD".to_string(),
        "ETH/USD".to_string(),
        "SOL/USD".to_string(),
    ];

    // Start Kraken WebSocket client in background
    let manager_clone = manager.clone();
    let symbols_clone = symbols.clone();
    tokio::spawn(async move {
        if let Err(e) = start_kraken_client(manager_clone, symbols_clone).await {
            tracing::error!("Kraken client error: {}", e);
        }
    });

    // Set up web server routes
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "OPTIONS"]);

    // GET /api/orderbook/:symbol - Get current orderbook
    let manager_current = manager.clone();
    let current_route = warp::path!("api" / "orderbook" / String)
        .and(warp::get())
        .map(move |symbol: String| {
            let manager = manager_current.clone();
            if let Some(snapshot) = manager.get_current(&symbol) {
                warp::reply::json(&snapshot)
            } else {
                warp::reply::json(&serde_json::json!({
                    "error": "Symbol not found"
                }))
            }
        });

    // GET /api/orderbook/:symbol/history?from=<ts>&to=<ts> - Get history
    let manager_history = manager.clone();
    let history_route = warp::path!("api" / "orderbook" / String / "history")
        .and(warp::get())
        .and(warp::query::<HistoryQuery>())
        .map(move |symbol: String, query: HistoryQuery| {
            let manager = manager_history.clone();

            let from = query
                .from
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|| Utc::now() - chrono::Duration::hours(24));

            let to = query
                .to
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            match manager.get_history(&symbol, from, to) {
                Ok(snapshots) => warp::reply::json(&snapshots),
                Err(e) => warp::reply::json(&serde_json::json!({
                    "error": format!("Failed to get history: {}", e)
                })),
            }
        });

    // GET /api/orderbook/:symbol/snapshot/:timestamp - Get snapshot at time
    let manager_snapshot = manager.clone();
    let snapshot_route = warp::path!("api" / "orderbook" / String / "snapshot" / String)
        .and(warp::get())
        .map(move |symbol: String, timestamp: String| {
            let manager = manager_snapshot.clone();

            if let Ok(dt) = DateTime::parse_from_rfc3339(&timestamp) {
                let dt_utc = dt.with_timezone(&Utc);
                match manager.get_at_time(&symbol, dt_utc) {
                    Ok(Some(snapshot)) => warp::reply::json(&snapshot),
                    Ok(None) => warp::reply::json(&serde_json::json!({
                        "error": "No snapshot found at that time"
                    })),
                    Err(e) => warp::reply::json(&serde_json::json!({
                        "error": format!("Failed to get snapshot: {}", e)
                    })),
                }
            } else {
                warp::reply::json(&serde_json::json!({
                    "error": "Invalid timestamp format"
                }))
            }
        });

    // GET /api/orderbook/:symbol/stats - Get storage stats
    let manager_stats = manager.clone();
    let stats_route = warp::path!("api" / "orderbook" / String / "stats")
        .and(warp::get())
        .map(move |symbol: String| {
            let manager = manager_stats.clone();
            match manager.get_stats(&symbol) {
                Ok(stats) => warp::reply::json(&stats),
                Err(e) => warp::reply::json(&serde_json::json!({
                    "error": format!("Failed to get stats: {}", e)
                })),
            }
        });

    // WebSocket route - ws://localhost:3033/ws/orderbook/:symbol
    let manager_ws = manager.clone();
    let ws_route = warp::path!("ws" / "orderbook" / String)
        .and(warp::ws())
        .map(move |symbol: String, ws: warp::ws::Ws| {
            let manager = manager_ws.clone();
            ws.on_upgrade(move |socket| websocket_handler(socket, symbol, manager))
        });

    // Health check
    let health_route = warp::path!("api" / "health")
        .and(warp::get())
        .map(|| {
            warp::reply::json(&serde_json::json!({
                "status": "healthy",
                "service": "orderbook-visualizer",
                "timestamp": Utc::now().to_rfc3339()
            }))
        });

    // Combine routes
    let routes = current_route
        .or(history_route)
        .or(snapshot_route)
        .or(stats_route)
        .or(ws_route)
        .or(health_route)
        .with(cors);

    tracing::info!("🌐 Server starting on http://localhost:3033");
    tracing::info!("📊 API endpoint: http://localhost:3033/api/orderbook/:symbol");
    tracing::info!("🔌 WebSocket: ws://localhost:3033/ws/orderbook/:symbol");

    warp::serve(routes).run(([127, 0, 0, 1], 3033)).await;

    Ok(())
}

/// WebSocket handler for real-time orderbook updates
async fn websocket_handler(
    ws: warp::ws::WebSocket,
    symbol: String,
    manager: Arc<OrderbookManager>,
) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let mut update_rx = manager.subscribe_updates();

    tracing::info!("WebSocket client connected for symbol: {}", symbol);

    // Send current snapshot on connection
    if let Some(snapshot) = manager.get_current(&symbol) {
        let msg = WsMessage::Snapshot { data: snapshot };
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = ws_tx.send(warp::ws::Message::text(json)).await;
        }
    }

    // Handle updates and client messages
    tokio::select! {
        _ = async {
            while let Ok(snapshot) = update_rx.recv().await {
                // Only send updates for the requested symbol
                if snapshot.symbol == symbol {
                    let msg = WsMessage::Snapshot { data: snapshot };
                    if let Ok(json) = serde_json::to_string(&msg) {
                        if ws_tx.send(warp::ws::Message::text(json)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        } => {},
        _ = async {
            while let Some(result) = ws_rx.next().await {
                match result {
                    Ok(msg) => {
                        if msg.is_close() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
        } => {},
    }

    tracing::info!("WebSocket client disconnected for symbol: {}", symbol);
}
