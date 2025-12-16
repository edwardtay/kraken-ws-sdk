// Kraken WebSocket SDK Demo - Enhanced Frontend

// Connection reason constants (must be defined before use)
const connectionReasons = {
    SERVER_CLOSE: 'server close',
    TIMEOUT: 'timeout',
    CLIENT_FORCED: 'client forced',
    NETWORK: 'network error',
    FAULT_INJECTION: 'fault injection',
    UNKNOWN: 'unknown'
};

class KrakenDemo {
    constructor() {
        this.ws = null;
        this.marketData = new Map();
        this.priceHistory = new Map();
        this.isConnected = false;
        this.updatesPaused = false;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 5;
        this.messageCount = 0;
        this.lastMessageTime = Date.now();
        this.trades = [];
        // Backpressure tracking
        this.totalDropped = 0;
        this.totalCoalesced = 0;
        this.totalReceived = 0;
        
        // Latency tracking
        this.latencySamples = [];
        this.maxLatencySamples = 1000;
        this.latencyHistogramChart = null;
        
        this.init();
    }
    
    init() {
        this.connectWebSocket();
        this.setupEventListeners();
        this.startHeartbeat();
        this.startStatsUpdate();
        this.initLatencyHistogram();
    }
    
    connectWebSocket() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/ws`;
        
        console.log('Connecting to WebSocket:', wsUrl);
        
        try {
            this.ws = new WebSocket(wsUrl);
            
            this.ws.onopen = () => {
                console.log('WebSocket connected');
                this.isConnected = true;
                this.reconnectAttempts = 0;
                this.lastDisconnectReason = null;
                this.updateConnectionStatus('Connected', true);
                sdkState.updateConnection('CONNECTED');
                if (typeof healthMonitor !== 'undefined') {
                    healthMonitor.updateConnection(true);
                    healthMonitor.resetSnapshotConsistency();
                }
            };
            
            this.ws.onmessage = (event) => {
                this.messageCount++;
                this.lastMessageTime = Date.now();
                if (!this.updatesPaused) {
                    this.handleMessage(event.data);
                }
            };
            
            this.ws.onclose = (event) => {
                console.log('WebSocket disconnected', event.code, event.reason);
                this.isConnected = false;
                
                // Determine disconnect reason
                let reason = connectionReasons.UNKNOWN;
                if (this.lastDisconnectReason === 'fault') {
                    reason = connectionReasons.FAULT_INJECTION;
                } else if (event.code === 1000) {
                    reason = connectionReasons.SERVER_CLOSE;
                } else if (event.code === 1006) {
                    reason = connectionReasons.NETWORK;
                } else if (event.code === 1001) {
                    reason = connectionReasons.CLIENT_FORCED;
                }
                
                if (typeof logDisconnect !== 'undefined') {
                    logDisconnect(reason, event.code);
                }
                
                this.updateConnectionStatus('Disconnected', false);
                sdkState.updateConnection('DISCONNECTED');
                if (typeof healthMonitor !== 'undefined') {
                    healthMonitor.updateConnection(false);
                }
                this.scheduleReconnect(reason);
            };
            
            this.ws.onerror = (error) => {
                console.error('WebSocket error:', error);
                this.updateConnectionStatus('Error', false);
                if (typeof healthMonitor !== 'undefined') {
                    healthMonitor.updateConnection(false);
                }
            };
            
        } catch (error) {
            console.error('Failed to create WebSocket:', error);
            this.updateConnectionStatus('Failed', false);
        }
    }
    
    handleMessage(data) {
        try {
            const marketData = JSON.parse(data);
            
            // Record raw frame for inspector
            frameInspector.addFrame(data, marketData);
            
            // Record for replay if recording
            if (typeof recorder !== 'undefined') {
                recorder.addFrame(data, marketData);
            }
            
            // Health monitoring - check sequence and lag
            if (typeof healthMonitor !== 'undefined') {
                if (marketData.sequence) {
                    healthMonitor.checkSequence(marketData.symbol, marketData.sequence);
                }
                if (marketData.exchange_timestamp) {
                    healthMonitor.checkLag(marketData.exchange_timestamp);
                }
            }
            
            this.updateMarketData(marketData);
            this.updateLastUpdateTime();
            
            // SDK-driven alerts - triggered directly from message events
            if (marketData.symbol && marketData.last_price) {
                sdkAlerts.checkPrice(
                    marketData.symbol,
                    marketData.last_price,
                    marketData.exchange_timestamp,
                    marketData.messages_received || this.messageCount
                );
            }
            
            this.addPriceToHistory(marketData);
            this.recordLatency(marketData);
        } catch (error) {
            console.error('Failed to parse message:', error);
            frameInspector.addFrame(data, { error: error.message });
        }
    }
    
    updateMarketData(data) {
        const oldData = this.marketData.get(data.symbol);
        this.marketData.set(data.symbol, data);
        this.renderMarketCard(data, oldData);
        document.getElementById('activeSymbols').textContent = this.marketData.size;
        
        // Update SDK state - track subscriptions
        if (data.symbol && !sdkState.subscriptions.has(data.symbol)) {
            sdkState.addSubscription(data.symbol, 'active');
        }
        sdkState.updateSubscription(data.symbol);
        
        // Update backpressure stats
        this.totalReceived++;
        if (data.messages_dropped) {
            const newDropped = data.messages_dropped - this.totalDropped;
            if (newDropped > 0) sdkState.incrementDropped(newDropped);
            this.totalDropped = data.messages_dropped;
        }
        if (data.messages_coalesced) {
            this.totalCoalesced = data.messages_coalesced;
        }
        document.getElementById('droppedCount').textContent = this.totalDropped;
        document.getElementById('coalescedCount').textContent = this.totalCoalesced;
        
        // Update queue size (simulated based on message rate)
        sdkState.setQueueSize(Math.max(0, Math.floor(this.messageCount / 10) % 50));
    }
    
    addPriceToHistory(data) {
        if (!data.last_price) return;
        
        let history = this.priceHistory.get(data.symbol) || [];
        history.push({
            time: new Date(),
            price: parseFloat(data.last_price)
        });
        
        // Keep last 50 data points
        if (history.length > 50) history.shift();
        this.priceHistory.set(data.symbol, history);
    }
    
    renderMarketCard(data, oldData) {
        const grid = document.getElementById('marketGrid');
        let card = document.getElementById(`card-${data.symbol.replace('/', '-')}`);
        
        if (!card) {
            card = this.createMarketCard(data);
            grid.appendChild(card);
        } else {
            this.updateMarketCard(card, data, oldData);
        }
    }
    
    getSymbolIcon(symbol) {
        const icons = {
            'BTC/USD': '₿',
            'ETH/USD': 'Ξ',
            'ADA/USD': '₳',
            'SOL/USD': '◎',
            'DOT/USD': '●'
        };
        return icons[symbol] || '💰';
    }
    
    createMarketCard(data) {
        const card = document.createElement('div');
        card.className = 'market-card';
        card.id = `card-${data.symbol.replace('/', '-')}`;
        
        card.innerHTML = `
            <div class="market-header">
                <div class="symbol">
                    <span class="symbol-icon">${this.getSymbolIcon(data.symbol)}</span>
                    ${data.symbol}
                </div>
                <div class="price-display">
                    <div class="current-price" data-price="${data.last_price || '0'}">
                        $${this.formatPrice(data.last_price)}
                    </div>
                    <span class="price-change up">+0.00%</span>
                </div>
            </div>
            
            <div class="market-stats">
                <div class="market-stat">
                    <div class="market-stat-label">Bid</div>
                    <div class="market-stat-value bid">${this.formatPrice(data.bid)}</div>
                </div>
                <div class="market-stat">
                    <div class="market-stat-label">Ask</div>
                    <div class="market-stat-value ask">${this.formatPrice(data.ask)}</div>
                </div>
                <div class="market-stat">
                    <div class="market-stat-label">Spread</div>
                    <div class="market-stat-value">${this.formatPrice(data.spread)}</div>
                </div>
                <div class="market-stat">
                    <div class="market-stat-label">Volume</div>
                    <div class="market-stat-value">${this.formatVolume(data.volume)}</div>
                </div>
            </div>
            
            <div class="mini-chart">
                <canvas id="chart-${data.symbol.replace('/', '-')}"></canvas>
            </div>
        `;
        
        // Initialize mini chart
        setTimeout(() => this.initMiniChart(data.symbol), 100);
        
        return card;
    }
    
    initMiniChart(symbol) {
        const canvasId = `chart-${symbol.replace('/', '-')}`;
        const canvas = document.getElementById(canvasId);
        if (!canvas) return;
        
        const ctx = canvas.getContext('2d');
        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    data: [],
                    borderColor: '#4dabf7',
                    borderWidth: 2,
                    fill: true,
                    backgroundColor: 'rgba(77, 171, 247, 0.1)',
                    tension: 0.4,
                    pointRadius: 0
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: { legend: { display: false } },
                scales: {
                    x: { display: false },
                    y: { display: false }
                },
                animation: { duration: 0 }
            }
        });
        
        // Store chart reference
        canvas.chartInstance = chart;
    }
    
    updateMarketCard(card, data, oldData) {
        const priceElement = card.querySelector('.current-price');
        const oldPrice = parseFloat(priceElement.dataset.price || '0');
        const newPrice = parseFloat(data.last_price || '0');
        
        if (newPrice !== oldPrice) {
            priceElement.dataset.price = newPrice;
            priceElement.textContent = `$${this.formatPrice(data.last_price)}`;
            
            // Update card border color based on price direction
            card.classList.remove('price-up', 'price-down');
            if (newPrice > oldPrice) {
                card.classList.add('price-up');
            } else if (newPrice < oldPrice) {
                card.classList.add('price-down');
            }
            
            // Update price change indicator
            const changeElement = card.querySelector('.price-change');
            if (oldPrice > 0) {
                const change = ((newPrice - oldPrice) / oldPrice * 100).toFixed(2);
                changeElement.textContent = `${change >= 0 ? '+' : ''}${change}%`;
                changeElement.className = `price-change ${change >= 0 ? 'up' : 'down'}`;
            }
        }
        
        // Update stats
        card.querySelector('.market-stat-value.bid').textContent = this.formatPrice(data.bid);
        card.querySelector('.market-stat-value.ask').textContent = this.formatPrice(data.ask);
        card.querySelectorAll('.market-stat-value')[2].textContent = this.formatPrice(data.spread);
        card.querySelectorAll('.market-stat-value')[3].textContent = this.formatVolume(data.volume);
        
        // Update mini chart
        this.updateMiniChart(data.symbol);
        
        // Add to trades if there's a trade
        if (data.last_trade) {
            this.addTrade(data.symbol, data.last_trade);
        }
    }
    
    updateMiniChart(symbol) {
        const canvasId = `chart-${symbol.replace('/', '-')}`;
        const canvas = document.getElementById(canvasId);
        if (!canvas || !canvas.chartInstance) return;
        
        const history = this.priceHistory.get(symbol) || [];
        const chart = canvas.chartInstance;
        
        chart.data.labels = history.map(h => h.time);
        chart.data.datasets[0].data = history.map(h => h.price);
        chart.update('none');
    }
    
    addTrade(symbol, trade) {
        this.trades.unshift({ symbol, ...trade, time: new Date() });
        if (this.trades.length > 10) this.trades.pop();
        this.renderTrades();
    }
    
    renderTrades() {
        const list = document.getElementById('activityList');
        list.innerHTML = this.trades.map(trade => `
            <div class="activity-item">
                <div>
                    <span class="activity-type ${trade.side?.toLowerCase() || 'buy'}">${trade.side || 'BUY'}</span>
                    <span style="margin-left: 8px;">${trade.symbol}</span>
                </div>
                <div style="text-align: right;">
                    <div style="font-weight: 600;">$${this.formatPrice(trade.price)}</div>
                    <div style="font-size: 0.75rem; color: var(--text-secondary);">${trade.volume || '0.00'}</div>
                </div>
            </div>
        `).join('');
    }
    
    formatPrice(price) {
        if (!price) return '0.00';
        const num = parseFloat(price);
        if (num >= 1000) return num.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
        if (num >= 1) return num.toFixed(4);
        return num.toFixed(6);
    }
    
    formatVolume(volume) {
        if (!volume) return '0.00';
        const num = parseFloat(volume);
        if (num >= 1000000) return (num / 1000000).toFixed(2) + 'M';
        if (num >= 1000) return (num / 1000).toFixed(2) + 'K';
        return num.toFixed(2);
    }
    
    updateConnectionStatus(status, connected) {
        document.getElementById('connectionStatus').textContent = status;
        const indicator = document.getElementById('statusIndicator');
        indicator.classList.toggle('disconnected', !connected);
    }
    
    updateLastUpdateTime() {
        document.getElementById('lastUpdate').textContent = new Date().toLocaleTimeString();
    }
    
    startStatsUpdate() {
        setInterval(() => {
            const elapsed = (Date.now() - this.lastMessageTime) / 1000;
            const rate = elapsed > 0 ? Math.round(this.messageCount / Math.max(elapsed, 1)) : 0;
            document.getElementById('messagesPerSec').textContent = rate;
        }, 1000);
    }
    
    scheduleReconnect(reason = 'unknown') {
        if (this.reconnectAttempts < this.maxReconnectAttempts) {
            this.reconnectAttempts++;
            sdkState.incrementReconnect();
            sdkState.updateConnection('RECONNECTING');
            
            if (typeof logReconnect !== 'undefined') {
                logReconnect(reason, this.reconnectAttempts);
            }
            
            const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
            this.updateConnectionStatus(`Reconnecting in ${Math.ceil(delay/1000)}s...`, false);
            setTimeout(() => this.connectWebSocket(), delay);
        } else {
            this.updateConnectionStatus('Connection failed', false);
            sdkState.updateConnection('FAILED');
            if (typeof healthMonitor !== 'undefined') {
                healthMonitor.updateConnection(false);
            }
        }
    }
    
    setupEventListeners() {
        window.addEventListener('beforeunload', () => { if (this.ws) this.ws.close(); });
        
        // Request notification permission
        if ('Notification' in window && Notification.permission === 'default') {
            Notification.requestPermission();
        }
    }
    
    startHeartbeat() {
        setInterval(() => {
            if (this.ws && this.ws.readyState === WebSocket.OPEN) {
                this.ws.send(JSON.stringify({ type: 'heartbeat' }));
            }
        }, 30000);
    }
    
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // LATENCY TRACKING
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    
    initLatencyHistogram() {
        const canvas = document.getElementById('latencyHistogram');
        if (!canvas) return;
        
        const ctx = canvas.getContext('2d');
        this.latencyHistogramChart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: [],
                datasets: [{
                    label: 'Messages',
                    data: [],
                    backgroundColor: 'rgba(77, 171, 247, 0.6)',
                    borderColor: '#4dabf7',
                    borderWidth: 1
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: { legend: { display: false } },
                scales: {
                    x: {
                        display: true,
                        grid: { display: false },
                        ticks: { color: '#8892a0', font: { size: 8 } }
                    },
                    y: {
                        display: false
                    }
                },
                animation: { duration: 0 }
            }
        });
    }
    
    recordLatency(data) {
        // Calculate latency from exchange timestamp
        if (data.exchange_timestamp || data.timestamp) {
            const exchangeTime = new Date(data.exchange_timestamp || data.timestamp).getTime();
            const receiveTime = Date.now();
            const latencyMs = receiveTime - exchangeTime;
            
            // Only record reasonable latencies (0-10 seconds)
            if (latencyMs >= 0 && latencyMs < 10000) {
                this.latencySamples.push(latencyMs);
                
                // Keep only recent samples
                if (this.latencySamples.length > this.maxLatencySamples) {
                    this.latencySamples.shift();
                }
                
                // Update stats every 10 messages
                if (this.latencySamples.length % 10 === 0) {
                    this.updateLatencyStats();
                }
            }
        }
    }
    
    updateLatencyStats() {
        if (this.latencySamples.length < 5) return;
        
        const sorted = [...this.latencySamples].sort((a, b) => a - b);
        const len = sorted.length;
        
        // Calculate percentiles
        const p50 = sorted[Math.floor(len * 0.50)];
        const p95 = sorted[Math.floor(len * 0.95)];
        const p99 = sorted[Math.floor(len * 0.99)];
        const max = sorted[len - 1];
        const min = sorted[0];
        
        // Calculate mean and stddev
        const sum = sorted.reduce((a, b) => a + b, 0);
        const mean = sum / len;
        const variance = sorted.reduce((acc, val) => acc + Math.pow(val - mean, 2), 0) / len;
        const stddev = Math.sqrt(variance);
        
        // Update UI
        document.getElementById('latencyP95').textContent = this.formatLatency(p95);
        document.getElementById('latencyP50').textContent = this.formatLatency(p50);
        document.getElementById('latencyP95Detail').textContent = this.formatLatency(p95);
        document.getElementById('latencyP99').textContent = this.formatLatency(p99);
        document.getElementById('latencyMax').textContent = this.formatLatency(max);
        document.getElementById('latencySamples').textContent = len.toLocaleString();
        document.getElementById('latencyMean').textContent = 
            `${this.formatLatency(mean)} ± ${this.formatLatency(stddev)}`;
        
        // Update histogram
        this.updateLatencyHistogram(sorted);
    }
    
    updateLatencyHistogram(sorted) {
        if (!this.latencyHistogramChart) return;
        
        // Create histogram buckets (0-5ms, 5-10ms, 10-20ms, 20-50ms, 50-100ms, 100ms+)
        const buckets = [
            { label: '0-5', min: 0, max: 5, count: 0 },
            { label: '5-10', min: 5, max: 10, count: 0 },
            { label: '10-20', min: 10, max: 20, count: 0 },
            { label: '20-50', min: 20, max: 50, count: 0 },
            { label: '50-100', min: 50, max: 100, count: 0 },
            { label: '100+', min: 100, max: Infinity, count: 0 }
        ];
        
        for (const latency of sorted) {
            for (const bucket of buckets) {
                if (latency >= bucket.min && latency < bucket.max) {
                    bucket.count++;
                    break;
                }
            }
        }
        
        this.latencyHistogramChart.data.labels = buckets.map(b => b.label + 'ms');
        this.latencyHistogramChart.data.datasets[0].data = buckets.map(b => b.count);
        
        // Color code by latency
        this.latencyHistogramChart.data.datasets[0].backgroundColor = buckets.map(b => {
            if (b.max <= 10) return 'rgba(0, 212, 170, 0.6)';  // Green
            if (b.max <= 50) return 'rgba(77, 171, 247, 0.6)'; // Blue
            if (b.max <= 100) return 'rgba(255, 169, 77, 0.6)'; // Orange
            return 'rgba(255, 107, 107, 0.6)'; // Red
        });
        
        this.latencyHistogramChart.update('none');
    }
    
    formatLatency(ms) {
        if (ms < 1) return '<1ms';
        if (ms < 1000) return `${Math.round(ms)}ms`;
        return `${(ms / 1000).toFixed(2)}s`;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SDK INTERNAL STATE TRACKER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const sdkState = {
    connectionState: 'INIT',
    subscriptions: new Map(), // symbol -> { status, subscribedAt, lastMessage }
    queueSize: 0,
    reconnectCount: 0,
    droppedMessages: 0,
    startTime: Date.now(),
    lastHeartbeat: null,
    pendingMessages: 0,
    processedMessages: 0,
    stateHistory: [],
    
    updateConnection(state) {
        this.connectionState = state;
        this.addStateEvent('connection', state);
        this.render();
    },
    
    addSubscription(symbol, status = 'active') {
        this.subscriptions.set(symbol, {
            status,
            subscribedAt: new Date(),
            lastMessage: null,
            messageCount: 0
        });
        this.addStateEvent('subscribe', symbol);
        this.render();
    },
    
    updateSubscription(symbol) {
        const sub = this.subscriptions.get(symbol);
        if (sub) {
            sub.lastMessage = new Date();
            sub.messageCount++;
        }
    },
    
    removeSubscription(symbol) {
        this.subscriptions.delete(symbol);
        this.addStateEvent('unsubscribe', symbol);
        this.render();
    },
    
    incrementReconnect() {
        this.reconnectCount++;
        this.addStateEvent('reconnect', `attempt #${this.reconnectCount}`);
        this.render();
    },
    
    incrementDropped(count = 1) {
        this.droppedMessages += count;
        this.render();
    },
    
    setQueueSize(size) {
        this.queueSize = size;
        this.render();
    },
    
    addStateEvent(type, detail) {
        this.stateHistory.unshift({
            time: new Date(),
            type,
            detail
        });
        if (this.stateHistory.length > 10) this.stateHistory.pop();
    },
    
    formatUptime() {
        const seconds = Math.floor((Date.now() - this.startTime) / 1000);
        if (seconds < 60) return `${seconds}s`;
        if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
        return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
    },
    
    render() {
        // Connection state
        const connEl = document.getElementById('sdkConnState');
        if (connEl) {
            connEl.textContent = this.connectionState;
            connEl.className = 'sdk-state-value ' + 
                (this.connectionState === 'CONNECTED' ? 'connected' : 
                 this.connectionState === 'DISCONNECTED' ? 'disconnected' : 
                 this.connectionState === 'RECONNECTING' ? 'warning' : 'info');
        }
        
        // Subscription count
        const subCountEl = document.getElementById('sdkSubCount');
        if (subCountEl) subCountEl.textContent = this.subscriptions.size;
        
        // Queue size
        const queueEl = document.getElementById('sdkQueueSize');
        if (queueEl) {
            queueEl.textContent = this.queueSize;
            queueEl.className = 'sdk-state-value' + (this.queueSize > 100 ? ' warning' : '');
        }
        
        // Reconnects
        const reconnEl = document.getElementById('sdkReconnects');
        if (reconnEl) {
            reconnEl.textContent = this.reconnectCount;
            reconnEl.className = 'sdk-state-value' + (this.reconnectCount > 0 ? ' warning' : '');
        }
        
        // Dropped
        const droppedEl = document.getElementById('sdkDropped');
        if (droppedEl) {
            droppedEl.textContent = this.droppedMessages;
            droppedEl.className = 'sdk-state-value' + (this.droppedMessages > 0 ? ' warning' : '');
        }
        
        // Uptime
        const uptimeEl = document.getElementById('sdkUptime');
        if (uptimeEl) uptimeEl.textContent = this.formatUptime();
        
        // Subscription tags
        const subsEl = document.getElementById('sdkSubscriptions');
        if (subsEl) {
            subsEl.innerHTML = Array.from(this.subscriptions.entries()).map(([symbol, data]) => {
                const statusClass = data.status === 'active' ? '' : ' pending';
                const msgCount = data.messageCount > 0 ? ` (${data.messageCount})` : '';
                return `<span class="sdk-sub-tag${statusClass}">${symbol}${msgCount}</span>`;
            }).join('');
        }
        
        // State details/history
        const detailsEl = document.getElementById('sdkStateDetails');
        if (detailsEl) {
            detailsEl.innerHTML = this.stateHistory.map(event => {
                const timeStr = event.time.toLocaleTimeString('en-US', { hour12: false });
                return `<div class="sdk-state-row">
                    <span style="color: var(--text-secondary)">${timeStr}</span>
                    <span style="color: var(--accent-blue)">${event.type}</span>
                    <span>${event.detail}</span>
                </div>`;
            }).join('');
        }
    }
};

// Update uptime every second
setInterval(() => {
    const uptimeEl = document.getElementById('sdkUptime');
    if (uptimeEl) uptimeEl.textContent = sdkState.formatUptime();
}, 1000);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// RAW WEBSOCKET FRAMES INSPECTOR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const frameInspector = {
    frames: [],
    maxFrames: 50,
    totalFrames: 0,
    totalBytes: 0,
    isPaused: false,
    showRaw: false,
    frameTimestamps: [],
    
    addFrame(rawData, parsed) {
        if (this.isPaused) return;
        
        const frame = {
            time: new Date(),
            raw: rawData,
            parsed: parsed,
            size: rawData.length,
            type: this.detectType(parsed)
        };
        
        this.frames.unshift(frame);
        if (this.frames.length > this.maxFrames) this.frames.pop();
        
        this.totalFrames++;
        this.totalBytes += frame.size;
        this.frameTimestamps.push(Date.now());
        
        // Keep only last 5 seconds of timestamps for rate calc
        const cutoff = Date.now() - 5000;
        this.frameTimestamps = this.frameTimestamps.filter(t => t > cutoff);
        
        this.render();
        this.updateStats();
    },
    
    detectType(parsed) {
        if (!parsed) return 'error';
        if (parsed.event === 'systemStatus' || parsed.event === 'subscriptionStatus') return 'system';
        if (parsed.symbol && (parsed.bid || parsed.ask || parsed.last_price)) return 'ticker';
        if (parsed.trades || parsed.last_trade) return 'trade';
        return 'system';
    },
    
    formatValue(value, depth = 0) {
        if (depth > 2) return '...';
        if (value === null) return '<span class="number">null</span>';
        if (typeof value === 'string') return `<span class="string">"${this.escapeHtml(value.substring(0, 50))}${value.length > 50 ? '...' : ''}"</span>`;
        if (typeof value === 'number') return `<span class="number">${value}</span>`;
        if (typeof value === 'boolean') return `<span class="number">${value}</span>`;
        if (Array.isArray(value)) {
            if (value.length === 0) return '[]';
            if (value.length > 3) return `[${this.formatValue(value[0], depth+1)}, ... +${value.length-1}]`;
            return `[${value.map(v => this.formatValue(v, depth+1)).join(', ')}]`;
        }
        if (typeof value === 'object') {
            const keys = Object.keys(value);
            if (keys.length === 0) return '{}';
            const preview = keys.slice(0, 3).map(k => 
                `<span class="key">${k}</span>: ${this.formatValue(value[k], depth+1)}`
            ).join(', ');
            return `{${preview}${keys.length > 3 ? ', ...' : ''}}`;
        }
        return String(value);
    },
    
    escapeHtml(str) {
        return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    },
    
    render() {
        const container = document.getElementById('wsFrames');
        if (!container) return;
        
        container.innerHTML = this.frames.map(frame => {
            const timeStr = frame.time.toLocaleTimeString('en-US', { hour12: false }) + 
                           '.' + frame.time.getMilliseconds().toString().padStart(3, '0');
            
            let dataHtml;
            if (this.showRaw) {
                dataHtml = `<span style="color: var(--text-secondary)">${this.escapeHtml(frame.raw.substring(0, 200))}${frame.raw.length > 200 ? '...' : ''}</span>`;
            } else {
                dataHtml = this.formatValue(frame.parsed);
            }
            
            return `
                <div class="ws-frame">
                    <span class="ws-frame-time">${timeStr}</span>
                    <span class="ws-frame-type ${frame.type}">${frame.type.toUpperCase()}</span>
                    <span class="ws-frame-data">${dataHtml}</span>
                </div>
            `;
        }).join('');
    },
    
    updateStats() {
        document.getElementById('frameCount').textContent = this.totalFrames.toLocaleString();
        document.getElementById('frameBytes').textContent = this.formatBytes(this.totalBytes);
        document.getElementById('frameRate').textContent = (this.frameTimestamps.length / 5).toFixed(1);
    },
    
    formatBytes(bytes) {
        if (bytes < 1024) return bytes + 'B';
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + 'KB';
        return (bytes / (1024 * 1024)).toFixed(2) + 'MB';
    },
    
    clear() {
        this.frames = [];
        this.render();
    },
    
    togglePause() {
        this.isPaused = !this.isPaused;
        const btn = document.querySelector('.ws-inspector button');
        if (btn) btn.textContent = this.isPaused ? '▶️ Resume' : '⏸️ Pause';
    },
    
    toggleRaw() {
        this.showRaw = !this.showRaw;
        this.render();
    }
};

function toggleFrameCapture() { frameInspector.togglePause(); }
function clearFrames() { frameInspector.clear(); }
function toggleRawJson() { frameInspector.toggleRaw(); }

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// FAULT INJECTION (Chaos Engineering)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const faultState = {
    disconnect: false,
    latency: false,
    packetLoss: false,
    reconnectCount: 0,
    resyncCount: 0,
    resubCount: 0,
    logEntries: []
};

function toggleFault(type) {
    faultState[type] = !faultState[type];
    const toggle = document.getElementById(`toggle${type.charAt(0).toUpperCase() + type.slice(1)}`);
    toggle.classList.toggle('active', faultState[type]);
    
    if (faultState[type]) {
        executeFault(type);
    } else {
        recoverFromFault(type);
    }
}

function executeFault(type) {
    const timestamp = new Date().toLocaleTimeString();
    
    switch (type) {
        case 'disconnect':
            addFaultLog(`[${timestamp}] 🔌 FAULT [SIMULATED]: Forcing disconnect...`, 'error');
            if (window.krakenDemo && window.krakenDemo.ws) {
                window.krakenDemo.lastDisconnectReason = 'fault';
                window.krakenDemo.ws.close();
                window.krakenDemo.updateConnectionStatus('Disconnected (Fault)', false);
            }
            // Auto-reconnect after 2 seconds
            setTimeout(() => {
                if (faultState.disconnect) {
                    faultState.reconnectCount++;
                    updateFaultIndicator('reconnect');
                    window.krakenDemo.connectWebSocket();
                    
                    // Simulate resubscribe
                    setTimeout(() => {
                        addFaultLog(`[${new Date().toLocaleTimeString()}] 📡 RESUBSCRIBE [SIMULATED]: Restoring subscriptions...`, 'warn');
                        faultState.resubCount++;
                        updateFaultIndicator('resub');
                        
                        setTimeout(() => {
                            addFaultLog(`[${new Date().toLocaleTimeString()}] ✅ RECOVERED: Connection restored, subscriptions active`, 'success');
                            highlightRecovery('reconnect');
                            highlightRecovery('resub');
                        }, 500);
                    }, 1000);
                }
            }, 2000);
            break;
            
        case 'latency':
            addFaultLog(`[${timestamp}] 🐢 FAULT [SIMULATED]: Injecting 500-1000ms latency...`, 'error');
            // Simulate high latency by delaying message processing
            if (window.krakenDemo) {
                window.krakenDemo._originalHandleMessage = window.krakenDemo.handleMessage.bind(window.krakenDemo);
                window.krakenDemo.handleMessage = function(data) {
                    const injectedDelay = 500 + Math.random() * 500;
                    setTimeout(() => {
                        this._originalHandleMessage(data);
                    }, injectedDelay);
                };
                // Mark health as degraded
                if (typeof healthMonitor !== 'undefined') {
                    healthMonitor.checks.lagUnderThreshold = false;
                    healthMonitor.evaluate();
                }
            }
            break;
            
        case 'packetLoss':
            addFaultLog(`[${timestamp}] 📦 FAULT [SIMULATED]: 30% packet loss enabled`, 'error');
            faultState.simulatedDropCount = 0;
            faultState.lastSeenSeq = 0;
            // Simulate packet loss by randomly dropping messages
            if (window.krakenDemo) {
                window.krakenDemo._originalHandleMessage2 = window.krakenDemo.handleMessage.bind(window.krakenDemo);
                window.krakenDemo.handleMessage = function(data) {
                    if (Math.random() > 0.3) { // 70% chance to process
                        this._originalHandleMessage2(data);
                        try {
                            const parsed = JSON.parse(data);
                            if (parsed.sequence) faultState.lastSeenSeq = parsed.sequence;
                        } catch (e) {}
                    } else {
                        // Dropped - log with details
                        faultState.simulatedDropCount++;
                        if (faultState.simulatedDropCount % 5 === 0) { // Log every 5th drop
                            const expectedSeq = faultState.lastSeenSeq + 1;
                            const gapSize = faultState.simulatedDropCount;
                            faultState.resyncCount++;
                            updateFaultIndicator('resync');
                            addFaultLog(
                                `[${new Date().toLocaleTimeString()}] 🔁 GAP [SIMULATED]: dropped ${gapSize} msgs, last seq: ${faultState.lastSeenSeq}`,
                                'warn'
                            );
                        }
                    }
                };
            }
            break;
    }
}

function recoverFromFault(type) {
    const timestamp = new Date().toLocaleTimeString();
    
    switch (type) {
        case 'disconnect':
            addFaultLog(`[${timestamp}] ✅ Disconnect fault disabled`, 'success');
            if (window.krakenDemo) {
                window.krakenDemo.lastDisconnectReason = null;
            }
            break;
            
        case 'latency':
            addFaultLog(`[${timestamp}] ✅ Latency injection disabled - restoring normal processing`, 'success');
            if (window.krakenDemo && window.krakenDemo._originalHandleMessage) {
                window.krakenDemo.handleMessage = window.krakenDemo._originalHandleMessage;
                delete window.krakenDemo._originalHandleMessage;
            }
            // Restore health check
            if (typeof healthMonitor !== 'undefined') {
                healthMonitor.checks.lagUnderThreshold = true;
                healthMonitor.evaluate();
            }
            break;
            
        case 'packetLoss':
            const droppedCount = faultState.simulatedDropCount || 0;
            addFaultLog(`[${timestamp}] ✅ Packet loss disabled - ${droppedCount} msgs were dropped`, 'success');
            if (window.krakenDemo && window.krakenDemo._originalHandleMessage2) {
                window.krakenDemo.handleMessage = window.krakenDemo._originalHandleMessage2;
                delete window.krakenDemo._originalHandleMessage2;
            }
            // Restore health check
            if (typeof healthMonitor !== 'undefined') {
                healthMonitor.checks.seqMonotonic = true;
                healthMonitor.evaluate();
            }
            faultState.simulatedDropCount = 0;
            break;
    }
}

function addFaultLog(message, type = 'info') {
    faultState.logEntries.unshift({ message, type, time: Date.now() });
    if (faultState.logEntries.length > 20) faultState.logEntries.pop();
    
    const logEl = document.getElementById('faultLog');
    if (logEl) {
        logEl.innerHTML = faultState.logEntries.map(entry => 
            `<div class="fault-log-entry ${entry.type}">${entry.message}</div>`
        ).join('');
    }
}

function updateFaultIndicator(type) {
    const countEl = document.getElementById(`${type}Count`);
    const indicatorEl = document.getElementById(`${type}Indicator`);
    
    if (countEl) {
        countEl.textContent = faultState[`${type}Count`];
    }
    if (indicatorEl) {
        indicatorEl.classList.add('active');
    }
}

function highlightRecovery(type) {
    const indicatorEl = document.getElementById(`${type}Indicator`);
    if (indicatorEl) {
        indicatorEl.classList.remove('active');
        indicatorEl.classList.add('recovered');
        setTimeout(() => indicatorEl.classList.remove('recovered'), 3000);
    }
}

// Global functions
function reconnectWebSocket() {
    if (window.krakenDemo) {
        window.krakenDemo.reconnectAttempts = 0;
        if (window.krakenDemo.ws) window.krakenDemo.ws.close();
        setTimeout(() => window.krakenDemo.connectWebSocket(), 100);
    }
}

function clearData() {
    document.getElementById('marketGrid').innerHTML = '';
    if (window.krakenDemo) {
        window.krakenDemo.marketData.clear();
        window.krakenDemo.priceHistory.clear();
    }
}

function toggleUpdates() {
    if (window.krakenDemo) {
        window.krakenDemo.updatesPaused = !window.krakenDemo.updatesPaused;
        const btn = document.querySelector('.header-center button:nth-child(2)');
        if (btn) btn.textContent = window.krakenDemo.updatesPaused ? '▶️ Resume' : '⏸️ Pause';
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PAIR SELECTOR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const pairManager = {
    activePairs: new Set(['BTC/USD', 'ETH/USD', 'ADA/USD']),
    
    addPair(pair) {
        if (!pair || this.activePairs.has(pair)) return;
        this.activePairs.add(pair);
        this.render();
        sdkAlerts.addStateEvent('pair_add', pair);
        // Note: In real implementation, this would send subscription to backend
        console.log(`📡 Subscribed to ${pair}`);
    },
    
    removePair(pair) {
        this.activePairs.delete(pair);
        this.render();
        sdkAlerts.addStateEvent('pair_remove', pair);
        // Remove market card
        const card = document.getElementById(`card-${pair.replace('/', '-')}`);
        if (card) card.remove();
        if (window.krakenDemo) {
            window.krakenDemo.marketData.delete(pair);
            window.krakenDemo.priceHistory.delete(pair);
        }
        sdkState.removeSubscription(pair);
    },
    
    render() {
        const container = document.getElementById('pairTags');
        if (!container) return;
        
        container.innerHTML = Array.from(this.activePairs).map(pair => `
            <span class="pair-tag active">
                ${pair}
                <span class="remove" onclick="event.stopPropagation(); pairManager.removePair('${pair}')">×</span>
            </span>
        `).join('');
    }
};

function addPairFromSelect() {
    const select = document.getElementById('pairSelect');
    if (select.value) {
        pairManager.addPair(select.value);
        select.value = '';
    }
}

function clearAllPairs() {
    const pairs = Array.from(pairManager.activePairs);
    pairs.forEach(pair => pairManager.removePair(pair));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SDK ALERTS (Real event-driven alerts)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const sdkAlerts = {
    alerts: [],
    sequenceId: 0,
    firedAlerts: [],
    maxFired: 20,
    
    init() {
        // Default alerts
        this.addAlert('BTC/USD', 'crosses', 90000);
        this.addAlert('BTC/USD', 'crosses', 85000);
        this.addAlert('ETH/USD', 'crosses', 3200);
        this.addAlert('ETH/USD', 'drops_below', 3000);
        this.render();
    },
    
    addAlert(symbol, condition, price) {
        const alert = {
            id: ++this.sequenceId,
            symbol,
            condition, // 'above', 'below', 'crosses', 'drops_below', 'rises_above'
            price,
            triggered: false,
            createdAt: new Date(),
            lastPrice: null
        };
        this.alerts.push(alert);
        this.render();
        return alert;
    },
    
    removeAlert(id) {
        this.alerts = this.alerts.filter(a => a.id !== id);
        this.render();
    },
    
    // Called from SDK message handler - this is the real event-driven check
    checkPrice(symbol, price, exchangeTimestamp, messageSeq) {
        const numPrice = parseFloat(price);
        
        this.alerts.forEach(alert => {
            if (alert.symbol !== symbol || alert.triggered) return;
            
            const lastPrice = alert.lastPrice;
            alert.lastPrice = numPrice;
            
            let shouldTrigger = false;
            let direction = '';
            
            switch (alert.condition) {
                case 'above':
                case 'rises_above':
                    if (numPrice >= alert.price && (!lastPrice || lastPrice < alert.price)) {
                        shouldTrigger = true;
                        direction = '📈';
                    }
                    break;
                case 'below':
                case 'drops_below':
                    if (numPrice <= alert.price && (!lastPrice || lastPrice > alert.price)) {
                        shouldTrigger = true;
                        direction = '📉';
                    }
                    break;
                case 'crosses':
                    if (lastPrice && ((numPrice >= alert.price && lastPrice < alert.price) ||
                                      (numPrice <= alert.price && lastPrice > alert.price))) {
                        shouldTrigger = true;
                        direction = numPrice > lastPrice ? '📈' : '📉';
                    }
                    break;
            }
            
            if (shouldTrigger) {
                this.fireAlert(alert, numPrice, exchangeTimestamp, messageSeq, direction);
            }
        });
    },
    
    fireAlert(alert, currentPrice, exchangeTimestamp, messageSeq, direction) {
        alert.triggered = true;
        
        const firedEvent = {
            alertId: alert.id,
            symbol: alert.symbol,
            condition: alert.condition,
            targetPrice: alert.price,
            actualPrice: currentPrice,
            direction,
            firedAt: new Date(),
            exchangeTimestamp: exchangeTimestamp || new Date().toISOString(),
            sequenceId: messageSeq || this.sequenceId++,
            latencyMs: exchangeTimestamp ? Date.now() - new Date(exchangeTimestamp).getTime() : null
        };
        
        this.firedAlerts.unshift(firedEvent);
        if (this.firedAlerts.length > this.maxFired) this.firedAlerts.pop();
        
        this.addStateEvent('alert_fired', `${alert.symbol} ${alert.condition} $${alert.price.toLocaleString()}`);
        this.render();
        
        // Browser notification
        if (Notification.permission === 'granted') {
            new Notification(`${direction} ${alert.symbol} Alert`, {
                body: `${alert.condition} $${alert.price.toLocaleString()} - Now: $${currentPrice.toLocaleString()}`,
                icon: '🦑',
                tag: `alert-${alert.id}`
            });
        }
        
        console.log(`🚨 ALERT FIRED: ${alert.symbol} ${alert.condition} $${alert.price} (seq: ${firedEvent.sequenceId})`);
    },
    
    addStateEvent(type, detail) {
        sdkState.addStateEvent(type, detail);
    },
    
    render() {
        const container = document.getElementById('alertsList');
        if (!container) return;
        
        // Show fired alerts first, then pending
        const fired = this.firedAlerts.slice(0, 5);
        const pending = this.alerts.filter(a => !a.triggered).slice(0, 5);
        
        let html = '';
        
        // Fired alerts
        fired.forEach(event => {
            const timeStr = event.firedAt.toLocaleTimeString('en-US', { hour12: false });
            const msStr = '.' + event.firedAt.getMilliseconds().toString().padStart(3, '0');
            html += `
                <div class="alert-item triggered">
                    <span class="alert-icon">${event.direction}</span>
                    <div class="alert-content">
                        <div class="alert-title">${event.symbol} ${event.condition} $${event.targetPrice.toLocaleString()}</div>
                        <div class="alert-time">Fired @ $${event.actualPrice.toLocaleString()} - ${timeStr}${msStr}</div>
                        <div class="alert-meta">
                            <span class="alert-seq">seq: ${event.sequenceId}</span> | 
                            latency: ${event.latencyMs ? event.latencyMs + 'ms' : 'N/A'}
                        </div>
                    </div>
                </div>
            `;
        });
        
        // Pending alerts
        pending.forEach(alert => {
            const conditionText = {
                'above': '↑ above',
                'below': '↓ below', 
                'crosses': '↔ crosses',
                'drops_below': '↓ drops below',
                'rises_above': '↑ rises above'
            }[alert.condition] || alert.condition;
            
            html += `
                <div class="alert-item pending">
                    <span class="alert-icon">⏳</span>
                    <div class="alert-content">
                        <div class="alert-title">${alert.symbol} ${conditionText} $${alert.price.toLocaleString()}</div>
                        <div class="alert-time">Waiting... ${alert.lastPrice ? '(last: $' + alert.lastPrice.toLocaleString() + ')' : ''}</div>
                        <div class="alert-meta">
                            <span class="alert-seq">id: ${alert.id}</span>
                            <span class="remove" style="cursor:pointer; margin-left: 8px;" onclick="sdkAlerts.removeAlert(${alert.id})">×</span>
                        </div>
                    </div>
                </div>
            `;
        });
        
        if (!html) {
            html = '<div style="font-size: 0.7rem; color: var(--text-secondary); padding: 8px;">No alerts configured</div>';
        }
        
        container.innerHTML = html;
    }
};

function showAddAlertModal() {
    const symbol = prompt('Symbol (e.g., BTC/USD):', 'BTC/USD');
    if (!symbol) return;
    const condition = prompt('Condition (above, below, crosses):', 'crosses');
    if (!condition) return;
    const price = parseFloat(prompt('Price:', '90000'));
    if (isNaN(price)) return;
    
    sdkAlerts.addAlert(symbol.toUpperCase(), condition, price);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// HEALTH BADGE - Enterprise-grade health monitoring
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// HEALTH BADGE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const healthMonitor = {
    status: 'healthy',
    checks: { snapshotConsistent: true, seqMonotonic: true, lagUnderThreshold: true, connectionStable: true },
    lastSeqBySymbol: new Map(),
    gapCount: 0,
    lagThresholdMs: 100,
    lastCheckTime: Date.now(),
    
    checkSequence: function(symbol, seq) {
        if (!seq) return true;
        var lastSeq = this.lastSeqBySymbol.get(symbol);
        if (lastSeq !== undefined && seq <= lastSeq) { this.checks.seqMonotonic = false; this.gapCount++; return false; }
        this.lastSeqBySymbol.set(symbol, seq);
        return true;
    },
    checkLag: function(ts) {
        if (!ts) return;
        var lag = Date.now() - new Date(ts).getTime();
        queueLagTracker.addSample(lag);
        this.checks.lagUnderThreshold = lag < this.lagThresholdMs;
    },
    updateConnection: function(c) { this.checks.connectionStable = c; this.evaluate(); },
    resetSnapshotConsistency: function() { this.checks.snapshotConsistent = true; this.evaluate(); },
    evaluate: function() {
        var c = this.checks;
        if (!c.connectionStable) this.status = 'unhealthy';
        else if (!c.snapshotConsistent || !c.seqMonotonic) this.status = 'degraded';
        else if (!c.lagUnderThreshold) this.status = 'degraded';
        else this.status = 'healthy';
        this.render();
    },
    render: function() {
        var badge = document.getElementById('healthBadge');
        var statusEl = document.getElementById('healthStatus');
        if (!badge || !statusEl) return;
        badge.className = 'health-badge ' + this.status;
        statusEl.textContent = this.status.toUpperCase();
    },
    tick: function() {
        if (Date.now() - this.lastCheckTime > 5000) { this.checks.seqMonotonic = true; this.lastCheckTime = Date.now(); }
        this.evaluate();
    }
};
setInterval(function() { healthMonitor.tick(); }, 1000);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// QUEUE LAG TRACKER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const queueLagTracker = {
    samples: [],
    maxSamples: 100,
    addSample: function(lag) {
        this.samples.push({ time: Date.now(), lag: lag });
        if (this.samples.length > this.maxSamples) this.samples.shift();
        this.render();
    },
    getAvgLag: function() {
        if (this.samples.length === 0) return 0;
        var sum = 0; for (var i = 0; i < this.samples.length; i++) sum += this.samples[i].lag;
        return Math.round(sum / this.samples.length);
    },
    render: function() {
        var el = document.getElementById('queueLag');
        if (!el) return;
        var avg = this.getAvgLag();
        el.textContent = avg < 1000 ? avg + 'ms' : (avg / 1000).toFixed(1) + 's';
        el.style.color = avg < 50 ? 'var(--accent-green)' : avg < 100 ? 'var(--accent-blue)' : avg < 500 ? 'var(--accent-orange)' : 'var(--accent-red)';
    }
};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// RECORD/REPLAY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const recorder = {
    isRecording: false,
    recordedFrames: [],
    startTime: null,
    start: function() {
        this.isRecording = true; this.recordedFrames = []; this.startTime = Date.now(); this.updateUI();
        addFaultLog('[' + new Date().toLocaleTimeString() + '] ⏺ RECORDING: Started', 'info');
    },
    stop: function() {
        this.isRecording = false;
        addFaultLog('[' + new Date().toLocaleTimeString() + '] ⏹ STOPPED: ' + this.recordedFrames.length + ' frames', 'success');
        this.updateUI();
    },
    addFrame: function(raw, parsed) {
        if (!this.isRecording || this.recordedFrames.length >= 10000) return;
        this.recordedFrames.push({ timestamp: Date.now(), raw: raw, parsed: parsed });
    },
    download: function() {
        if (this.recordedFrames.length === 0) return;
        var blob = new Blob([JSON.stringify({ frames: this.recordedFrames }, null, 2)], { type: 'application/json' });
        var a = document.createElement('a'); a.href = URL.createObjectURL(blob);
        a.download = 'kraken-recording-' + Date.now() + '.json'; a.click();
    },
    updateUI: function() {
        var btn = document.getElementById('recordBtn');
        if (!btn) return;
        btn.classList.toggle('recording', this.isRecording);
        btn.textContent = this.isRecording ? '■ stop' : '● rec';
    }
};

function toggleRecording() {
    if (recorder.isRecording) { recorder.stop(); if (recorder.recordedFrames.length > 0 && confirm('Download?')) recorder.download(); }
    else recorder.start();
}

function logReconnect(reason, attempt) { addFaultLog('[' + new Date().toLocaleTimeString() + '] 🔌 RECONNECT #' + attempt + ': ' + reason, 'warn'); }
function logDisconnect(reason, code) { addFaultLog('[' + new Date().toLocaleTimeString() + '] ❌ DISCONNECT: ' + reason + (code ? ' (' + code + ')' : ''), 'error'); }


// Initialize on DOM ready
document.addEventListener('DOMContentLoaded', function() {
    window.krakenDemo = new KrakenDemo();
    pairManager.render();
    sdkAlerts.init();
    if (typeof healthMonitor !== 'undefined') healthMonitor.render();
    addFaultLog('[' + new Date().toLocaleTimeString() + '] 🚀 SDK console ready', 'info');
});
