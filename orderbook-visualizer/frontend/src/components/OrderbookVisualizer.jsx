import React, { useState, useEffect, useRef } from 'react';
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Cell } from 'recharts';
import { format } from 'date-fns';
import './OrderbookVisualizer.css';

const OrderbookVisualizer = ({
  symbol = 'BTC/USD',
  depth = 20,
  autoUpdate = true,
  timeTravel = false,
  startTime = null,
  endTime = null,
  playbackSpeed = 1.0,
  theme = 'dark',
  apiUrl = 'http://localhost:3033',
}) => {
  const [orderbook, setOrderbook] = useState(null);
  const [connected, setConnected] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  // Time travel state
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(null);
  const [history, setHistory] = useState([]);
  const [historyIndex, setHistoryIndex] = useState(0);

  const wsRef = useRef(null);
  const playbackTimerRef = useRef(null);

  // Connect to WebSocket for real-time updates
  useEffect(() => {
    if (!autoUpdate || timeTravel) return;

    const wsUrl = `ws://localhost:3033/ws/orderbook/${symbol}`;
    const ws = new WebSocket(wsUrl);

    ws.onopen = () => {
      console.log(`Connected to ${symbol} orderbook stream`);
      setConnected(true);
      setLoading(false);
    };

    ws.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        if (message.type === 'snapshot') {
          setOrderbook(message.data);
        } else if (message.type === 'error') {
          setError(message.message);
        }
      } catch (err) {
        console.error('Failed to parse message:', err);
      }
    };

    ws.onerror = (err) => {
      console.error('WebSocket error:', err);
      setError('WebSocket connection error');
      setConnected(false);
    };

    ws.onclose = () => {
      console.log('WebSocket closed');
      setConnected(false);
    };

    wsRef.current = ws;

    return () => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.close();
      }
    };
  }, [symbol, autoUpdate, timeTravel]);

  // Load historical data for time travel
  useEffect(() => {
    if (!timeTravel) return;

    const from = startTime || new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString();
    const to = endTime || new Date().toISOString();

    setLoading(true);
    fetch(`${apiUrl}/api/orderbook/${symbol}/history?from=${from}&to=${to}`)
      .then((res) => res.json())
      .then((data) => {
        if (Array.isArray(data)) {
          setHistory(data);
          if (data.length > 0) {
            setOrderbook(data[0]);
            setCurrentTime(new Date(data[0].timestamp));
          }
        }
        setLoading(false);
      })
      .catch((err) => {
        console.error('Failed to load history:', err);
        setError('Failed to load historical data');
        setLoading(false);
      });
  }, [symbol, timeTravel, startTime, endTime, apiUrl]);

  // Time travel playback
  useEffect(() => {
    if (!isPlaying || !timeTravel || history.length === 0) {
      if (playbackTimerRef.current) {
        clearInterval(playbackTimerRef.current);
      }
      return;
    }

    playbackTimerRef.current = setInterval(() => {
      setHistoryIndex((prev) => {
        const next = prev + 1;
        if (next >= history.length) {
          setIsPlaying(false);
          return prev;
        }
        setOrderbook(history[next]);
        setCurrentTime(new Date(history[next].timestamp));
        return next;
      });
    }, 1000 / playbackSpeed);

    return () => {
      if (playbackTimerRef.current) {
        clearInterval(playbackTimerRef.current);
      }
    };
  }, [isPlaying, timeTravel, history, playbackSpeed]);

  // Format orderbook data for visualization
  const formatOrderbookData = () => {
    if (!orderbook) return { bids: [], asks: [] };

    const bids = orderbook.bids
      .slice(0, depth)
      .map((level) => ({
        price: parseFloat(level.price),
        volume: parseFloat(level.volume),
        total: 0, // Will calculate cumulative
      }))
      .reverse(); // Reverse to show highest bid first

    const asks = orderbook.asks
      .slice(0, depth)
      .map((level) => ({
        price: parseFloat(level.price),
        volume: parseFloat(level.volume),
        total: 0,
      }));

    // Calculate cumulative volumes
    let cumBid = 0;
    bids.forEach((bid) => {
      cumBid += bid.volume;
      bid.total = cumBid;
    });

    let cumAsk = 0;
    asks.forEach((ask) => {
      cumAsk += ask.volume;
      ask.total = cumAsk;
    });

    return { bids, asks };
  };

  const { bids, asks } = formatOrderbookData();
  const spread = orderbook && asks[0] && bids[bids.length - 1]
    ? (asks[0].price - bids[bids.length - 1].price).toFixed(2)
    : 'N/A';

  const midPrice = orderbook && asks[0] && bids[bids.length - 1]
    ? ((asks[0].price + bids[bids.length - 1].price) / 2).toFixed(2)
    : 'N/A';

  // Time travel controls
  const handlePlayPause = () => setIsPlaying(!isPlaying);

  const handleSeek = (e) => {
    const index = parseInt(e.target.value);
    setHistoryIndex(index);
    if (history[index]) {
      setOrderbook(history[index]);
      setCurrentTime(new Date(history[index].timestamp));
    }
  };

  const handleStepBackward = () => {
    if (historyIndex > 0) {
      const newIndex = historyIndex - 1;
      setHistoryIndex(newIndex);
      setOrderbook(history[newIndex]);
      setCurrentTime(new Date(history[newIndex].timestamp));
    }
  };

  const handleStepForward = () => {
    if (historyIndex < history.length - 1) {
      const newIndex = historyIndex + 1;
      setHistoryIndex(newIndex);
      setOrderbook(history[newIndex]);
      setCurrentTime(new Date(history[newIndex].timestamp));
    }
  };

  if (loading) {
    return <div className={`orderbook-container ${theme}`}>Loading...</div>;
  }

  if (error) {
    return <div className={`orderbook-container ${theme}`}>Error: {error}</div>;
  }

  return (
    <div className={`orderbook-container ${theme}`}>
      {/* Header */}
      <div className="orderbook-header">
        <h2>{symbol} Orderbook</h2>
        <div className="orderbook-stats">
          <div className="stat">
            <span className="label">Mid Price:</span>
            <span className="value">${midPrice}</span>
          </div>
          <div className="stat">
            <span className="label">Spread:</span>
            <span className="value">${spread}</span>
          </div>
          {!timeTravel && (
            <div className="stat">
              <span className={`status ${connected ? 'connected' : 'disconnected'}`}>
                {connected ? '● Live' : '○ Disconnected'}
              </span>
            </div>
          )}
          {timeTravel && currentTime && (
            <div className="stat">
              <span className="label">Time:</span>
              <span className="value">{format(currentTime, 'yyyy-MM-dd HH:mm:ss')}</span>
            </div>
          )}
        </div>
      </div>

      {/* Time Travel Controls */}
      {timeTravel && history.length > 0 && (
        <div className="time-controls">
          <button onClick={handleStepBackward} disabled={historyIndex === 0}>
            ⏮ Step Back
          </button>
          <button onClick={handlePlayPause}>
            {isPlaying ? '⏸ Pause' : '▶ Play'}
          </button>
          <button onClick={handleStepForward} disabled={historyIndex >= history.length - 1}>
            Step Forward ⏭
          </button>
          <input
            type="range"
            min="0"
            max={history.length - 1}
            value={historyIndex}
            onChange={handleSeek}
            className="timeline-slider"
          />
          <span className="timeline-info">
            {historyIndex + 1} / {history.length}
          </span>
        </div>
      )}

      {/* Orderbook Visualization */}
      <div className="orderbook-content">
        {/* Asks (Sell Orders) */}
        <div className="orderbook-side asks-side">
          <h3>Asks (Sell)</h3>
          <ResponsiveContainer width="100%" height={300}>
            <BarChart data={asks} layout="vertical" margin={{ top: 5, right: 30, left: 20, bottom: 5 }}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis type="number" />
              <YAxis type="category" dataKey="price" tickFormatter={(val) => val.toFixed(2)} />
              <Tooltip
                formatter={(value, name) => [
                  name === 'volume' ? value.toFixed(4) : value.toFixed(2),
                  name === 'volume' ? 'Volume' : 'Cumulative',
                ]}
                labelFormatter={(price) => `Price: $${price.toFixed(2)}`}
              />
              <Bar dataKey="volume" stackId="a">
                {asks.map((entry, index) => (
                  <Cell key={`cell-${index}`} fill="#e74c3c" opacity={0.7} />
                ))}
              </Bar>
            </BarChart>
          </ResponsiveContainer>
        </div>

        {/* Spread Indicator */}
        <div className="spread-indicator">
          <div className="spread-label">Spread</div>
          <div className="spread-value">${spread}</div>
        </div>

        {/* Bids (Buy Orders) */}
        <div className="orderbook-side bids-side">
          <h3>Bids (Buy)</h3>
          <ResponsiveContainer width="100%" height={300}>
            <BarChart data={bids} layout="vertical" margin={{ top: 5, right: 30, left: 20, bottom: 5 }}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis type="number" />
              <YAxis type="category" dataKey="price" tickFormatter={(val) => val.toFixed(2)} />
              <Tooltip
                formatter={(value, name) => [
                  name === 'volume' ? value.toFixed(4) : value.toFixed(2),
                  name === 'volume' ? 'Volume' : 'Cumulative',
                ]}
                labelFormatter={(price) => `Price: $${price.toFixed(2)}`}
              />
              <Bar dataKey="volume" stackId="a">
                {bids.map((entry, index) => (
                  <Cell key={`cell-${index}`} fill="#27ae60" opacity={0.7} />
                ))}
              </Bar>
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* Depth Table */}
      <div className="orderbook-table">
        <div className="table-section">
          <h4>Bids</h4>
          <table>
            <thead>
              <tr>
                <th>Price</th>
                <th>Volume</th>
                <th>Total</th>
              </tr>
            </thead>
            <tbody>
              {bids.slice().reverse().slice(0, 10).map((bid, idx) => (
                <tr key={idx} className="bid-row">
                  <td className="price">${bid.price.toFixed(2)}</td>
                  <td>{bid.volume.toFixed(4)}</td>
                  <td>{bid.total.toFixed(4)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <div className="table-section">
          <h4>Asks</h4>
          <table>
            <thead>
              <tr>
                <th>Price</th>
                <th>Volume</th>
                <th>Total</th>
              </tr>
            </thead>
            <tbody>
              {asks.slice(0, 10).map((ask, idx) => (
                <tr key={idx} className="ask-row">
                  <td className="price">${ask.price.toFixed(2)}</td>
                  <td>{ask.volume.toFixed(4)}</td>
                  <td>{ask.total.toFixed(4)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};

export default OrderbookVisualizer;
