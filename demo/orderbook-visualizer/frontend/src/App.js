import React, { useState } from 'react';
import OrderbookVisualizer from './components/OrderbookVisualizer';
import './App.css';

function App() {
  const [symbol, setSymbol] = useState('BTC/USD');
  const [mode, setMode] = useState('live'); // 'live' or 'timetravel'
  const [depth, setDepth] = useState(20);

  // Time travel settings
  const [startTime, setStartTime] = useState('');
  const [endTime, setEndTime] = useState('');
  const [playbackSpeed, setPlaybackSpeed] = useState(1.0);

  const symbols = ['BTC/USD', 'ETH/USD', 'SOL/USD'];

  return (
    <div className="App">
      <header className="App-header">
        <h1>Kraken Orderbook Visualizer</h1>
        <p>Real-time and historical orderbook visualization with time travel</p>
      </header>

      <div className="controls-panel">
        <div className="control-group">
          <label>Symbol:</label>
          <select value={symbol} onChange={(e) => setSymbol(e.target.value)}>
            {symbols.map((sym) => (
              <option key={sym} value={sym}>
                {sym}
              </option>
            ))}
          </select>
        </div>

        <div className="control-group">
          <label>Mode:</label>
          <select value={mode} onChange={(e) => setMode(e.target.value)}>
            <option value="live">Live</option>
            <option value="timetravel">Time Travel</option>
          </select>
        </div>

        <div className="control-group">
          <label>Depth:</label>
          <input
            type="number"
            value={depth}
            onChange={(e) => setDepth(parseInt(e.target.value))}
            min="5"
            max="50"
          />
        </div>

        {mode === 'timetravel' && (
          <>
            <div className="control-group">
              <label>Start Time:</label>
              <input
                type="datetime-local"
                value={startTime}
                onChange={(e) => setStartTime(e.target.value)}
              />
            </div>

            <div className="control-group">
              <label>End Time:</label>
              <input
                type="datetime-local"
                value={endTime}
                onChange={(e) => setEndTime(e.target.value)}
              />
            </div>

            <div className="control-group">
              <label>Playback Speed:</label>
              <select value={playbackSpeed} onChange={(e) => setPlaybackSpeed(parseFloat(e.target.value))}>
                <option value="0.5">0.5x</option>
                <option value="1.0">1.0x</option>
                <option value="2.0">2.0x</option>
                <option value="5.0">5.0x</option>
                <option value="10.0">10.0x</option>
              </select>
            </div>
          </>
        )}
      </div>

      <main className="visualizer-container">
        <OrderbookVisualizer
          symbol={symbol}
          depth={depth}
          autoUpdate={mode === 'live'}
          timeTravel={mode === 'timetravel'}
          startTime={startTime ? new Date(startTime).toISOString() : null}
          endTime={endTime ? new Date(endTime).toISOString() : null}
          playbackSpeed={playbackSpeed}
          theme="dark"
        />
      </main>

      <footer className="App-footer">
        <p>
          Built with <a href="https://github.com/yourusername/kraken-ws-sdk" target="_blank" rel="noopener noreferrer">
            Kraken WebSocket SDK
          </a>
        </p>
        <p>Data provided by Kraken Exchange</p>
      </footer>
    </div>
  );
}

export default App;
