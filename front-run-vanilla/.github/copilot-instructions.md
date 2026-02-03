# Copilot Instructions for Front Run Vanilla

## Project Overview

**Front Run Vanilla** is a production-grade high-frequency trading (HFT) system for cryptocurrency markets. It detects large market orders through real-time order book analysis and executes trades with <100ms latency. Built in Rust for the Binance Futures exchange.

**Core Strategy:** Identify "whale" activity (>$500k orders) via statistical anomalies in bid/ask imbalance and aggressive order flow, then capitalize on predictable price impact (2-10 basis points per trade, 10-50 trades/day).

**Expected Performance:** Sharpe Ratio 1.5-2.5, Win Rate 50-60%, Average trade duration 1-3 seconds.

---

## Complete Project Structure

```
front-run-vanilla/
├── Cargo.toml                 # Dependencies, release optimizations, bin targets
├── Makefile                   # All development commands (make help)
├── README.md                  # Quick start & overview
├── GREYPAPER.md              # 900+ line technical specification (REQUIRED READING)
├── setup.sh                  # Initial environment setup
├── .env                      # API keys & credentials (DO NOT COMMIT)
│
├── config/                   # Environment-specific configurations
│   ├── backtest.toml        # Historical testing (aggressive parameters)
│   ├── paper_trading.toml   # Live data, simulated fills (recommended start)
│   └── production.toml      # Real money (tightest risk limits)
│
├── src/
│   ├── lib.rs               # Module declarations & public re-exports
│   ├── main.rs              # Stub entry point (use bin targets instead)
│   │
│   ├── data/                # Market data structures & order book
│   │   ├── mod.rs           # Module exports
│   │   ├── types.rs         # Core types: Side, Trade, Order, Signal, PriceLevel
│   │   └── orderbook.rs     # Lock-free OrderBook using DashMap (<1ms updates)
│   │
│   ├── exchange/            # Binance exchange integration
│   │   ├── mod.rs           # Module exports
│   │   └── binance/         # Binance-specific implementation
│   │       ├── mod.rs       # Binance module exports
│   │       ├── types.rs     # Binance API types (Depth, Trade, Order)
│   │       ├── websocket.rs # WebSocket stream handler (tokio-tungstenite)
│   │       ├── rest.rs      # REST API client (order execution, account info)
│   │       └── auth.rs      # HMAC-SHA256 signature generation
│   │
│   ├── strategy/            # Signal generation & execution logic
│   │   ├── mod.rs           # Module exports
│   │   ├── execution.rs     # ExecutionEngine: position sizing, entry/exit
│   │   └── signals/         # Three independent signal detectors
│   │       ├── mod.rs       # Module exports
│   │       ├── imbalance.rs # ImbalanceDetector: bid/ask volume ratios
│   │       ├── flow.rs      # FlowAnalyzer: aggressive order clustering
│   │       └── composite.rs # CompositeSignal: voting mechanism (2+ signals)
│   │
│   ├── risk/                # Risk management & position tracking
│   │   ├── mod.rs           # Module exports
│   │   ├── limits.rs        # RiskManager: position size, daily loss, latency checks
│   │   └── position.rs      # Position: entry/exit tracking, PnL calculation
│   │
│   ├── backtest/            # Historical backtesting engine
│   │   ├── mod.rs           # Module exports
│   │   └── engine.rs        # BacktestEngine: event replay, SimulatedFill
│   │
│   ├── utils/               # Utilities: config, logging, helpers
│   │   ├── mod.rs           # Module exports
│   │   ├── config.rs        # Config: loads TOML files into typed structures
│   │   └── logger.rs        # Logging: tracing setup, Prometheus metrics
│   │
│   └── bin/                 # Executable targets (cargo run --bin <name>)
│       ├── paper_trader.rs  # Paper trading: live data, simulated fills
│       ├── live_trader.rs   # LIVE TRADING: real money, Binance API (⚠️ DANGER)
│       ├── backtester.rs    # Backtesting: historical replay with metrics
│       └── data_collector.rs # Utility: collect historical market data
│
├── benches/                 # Performance benchmarks (criterion)
│   └── orderbook_bench.rs   # Benchmark order book operations
│
├── tests/                   # Integration tests
├── docs/                    # Additional documentation
├── logs/                    # Log files (gitignored)
├── data/                    # Historical market data (gitignored)
└── scripts/                 # Helper scripts

```

---

## Critical Architecture Pattern

**Data Flow Pipeline:**
```
Binance WebSocket → OrderBook Update → Signal Calculation → Risk Check → Order Execution
     (tokio task)      (DashMap)         (3 detectors)      (RiskManager)  (REST API)
        ↓                  ↓                   ↓                  ↓              ↓
   Live Updates      <1ms lock-free      <5ms per update    Circuit breaker  Market orders
   (100ms depth)     concurrent reads    composite voting    + position limits + fills
   (aggTrades)
```

**Key System Properties:**
- **Lock-free order book:** Uses `Arc<DashMap<Price, Quantity>>` for concurrent reads without blocking
- **Multi-signal voting:** Three independent detectors must confirm (configurable threshold, typically 2-3 of 3)
- **Position-aware execution:** Entry size varies 0.5x-2.0x base size based on signal confidence
- **Circuit breaker:** Automatic trading halt on daily loss, max drawdown, or latency threshold breach

**Core Components Responsibility Map:**

| Module | Responsibility | Hot Path? |
|--------|---|---|
| `OrderBook` | Track bid/ask depth, calculate imbalances | YES (every 100ms) |
| `ImbalanceDetector` | Z-score of bid/ask ratio history | YES |
| `FlowAnalyzer` | Aggressive buy/sell clustering | YES |
| `CompositeSignal` | Vote across signal detectors | YES |
| `ExecutionEngine` | Position sizing, order placement | YES |
| `RiskManager` | Circuit breaker, position limits | YES |
| `PositionManager` | Track open positions, calculate PnL | YES |
| `Config` | Load and validate parameters | NO |
| `BinanceWebSocket` | Stream depth/trade updates | YES (background) |
| `BinanceRestClient` | Place orders, query accounts | NO (blocking) |

---

## Essential Developer Workflows

**Build & Test:**
```bash
make build       # Release build (optimized with LTO, 3 opt levels)
make build-dev   # Debug build (faster compile)
make test        # Run all tests (release mode)
make bench       # Performance benchmarks (HTML report in target/criterion/)
make check       # Clippy lints
make lint        # Full linting + format check
make format      # Auto-format code
```

**Trading Modes:**
```bash
make paper       # Paper trading (SAFE: simulated fills, live order book)
make backtest    # Backtesting (historical data replay with metrics)
make live        # REAL TRADING (⚠️ DANGER: uses production.toml + API keys)
make watch       # Auto-rebuild on file changes
```

**Dev Setup:**
```bash
./setup.sh       # Install Rust, dependencies
make install     # Install dev tools (clippy, fmt, criterion)
nano .env        # Edit API credentials
make help        # Show all commands
```

---

## Project-Specific Conventions

### 1. **Fixed-Point Arithmetic (Decimal, NOT f64)**

**CRITICAL RULE:** All financial values use `rust_decimal::Decimal`. Never use `f64` for prices/quantities/PnL.

```rust
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// ✅ CORRECT: Prevent precision loss
let price = Decimal::from_str("100.50")?;
let quantity = dec!(10.5);
let fee = dec!(0.0004);  // 0.04%
let total = price * quantity * (dec!(1) - fee);

// ❌ WRONG: Floating-point errors compound
let price = 100.50_f64;  // Already imprecise
let total = price * 10.5 * 0.9996;  // Error multiplies

// ❌ WRONG: Why it matters
100.1_f64 + 0.2_f64 == 100.3_f64  // FALSE! This is why Decimal exists
```

Cargo.toml includes `rust_decimal` with `serde` feature for JSON serialization.

### 2. **Configuration via TOML with Environment Modes**

Three configuration files in `config/`:

```toml
# config/paper_trading.toml (RECOMMENDED START)
[general]
environment = "paper_trading"
symbol = "BTCUSDT"

[strategy]
imbalance_threshold = 3.0        # Z-score threshold
min_confirming_signals = 2       # 2 of 3 detectors must agree
lookback_window_ms = 5000        # 5 second lookback
take_profit_bps = 10             # Exit at +10 basis points
stop_loss_bps = 5                # Exit at -5 basis points
max_hold_time_ms = 5000          # Max 5 second position

[position_sizing]
base_notional_usd = 1000         # $1000 base position
min_size_multiplier = 0.5        # 0.5x when low confidence
max_size_multiplier = 2.0        # 2.0x when high confidence
```

Load via `Config::load()` in any binary:
```rust
let config = Config::load()?;  // Auto-finds paper_trading.toml
let symbol = config.general.symbol;
```

### 3. **Async/Await with Tokio + Lock-Free Concurrency**

Tokio features enabled in Cargo.toml: `["full"]` (macros, net, time, sync, io, rt, fs).

```rust
// ✅ WebSocket background task (spawned at startup)
let orderbook = Arc::new(OrderBook::new("BTCUSDT"));
tokio::spawn({
    let ob = Arc::clone(&orderbook);
    async move {
        ws.run().await;  // Never returns; infinite event loop
    }
});

// ✅ Signal calculation task (reads orderbook lock-free)
tokio::spawn({
    let ob = Arc::clone(&orderbook);
    async move {
        loop {
            let signal = signal_detector.calculate(&ob)?;  // No lock!
        }
    }
});

// ✅ Data sharing: Arc<DashMap> not Mutex
// DashMap allows concurrent reads without acquiring a lock
orderbook.update_level(Side::Buy, price, qty)?;  // Writer blocks writers only
let ratio = orderbook.calculate_imbalance(5)?;   // Reader never blocks

// ✅ Task communication: crossbeam channels
let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Signal>();
tokio::spawn(async move {
    while let Some(signal) = rx.recv().await {
        executor.execute_signal(signal).await;
    }
});
```

### 4. **Structured Logging with Tracing**

All logs use `tracing` crate (see `src/utils/logger.rs`):

```rust
use tracing::{info, warn, error, debug};

// Info: Normal operations
info!(symbol = "BTCUSDT", 
      imbalance = %3.14,     // % formats the value
      "Signal generated");

// Warn: Recoverable issues
warn!(position_size = %qty,
      reason = "Low confidence signal",
      "Reducing position size");

// Error: Failures that need attention
error!(error = %err,
       order_id = order_id,
       "Failed to execute order");

// Initialize at startup:
tracing_subscriber::fmt()
    .with_env_filter("info")  // RUST_LOG=debug to increase
    .with_target(false)
    .init();
```

Prometheus metrics also collected (see `logger.rs` for setup).

### 5. **Signal Composition via Voting Mechanism**

Three independent signal detectors vote on trade direction:

```rust
pub enum SignalComponent {
    Imbalance(ImbalanceStats),   // Bid/ask ratio z-score
    Flow(FlowStats),             // Order flow momentum
    Custom(CustomStats),         // Future extension point
}

pub struct CompositeSignal {
    pub components: Vec<SignalComponent>,
    pub confidence: f64,  // 0.0-1.0 based on agreement
    pub direction: Side,  // Buy or Sell
}

// Voting logic in SignalAggregator
let signal = aggregator.evaluate(
    &imbalance_result,
    &flow_result,
    &custom_result
)?;

// Typically requires min_confirming_signals = 2 of 3
// Confidence scales with agreement (all 3 agree = 1.0 confidence)
```

Adding a new signal: Create struct, implement calculation, add to `CompositeSignal` enum.

### 6. **Error Handling Strategy**

Two error types used throughout:

```rust
use anyhow::Result;  // For recoverable errors (try-catch-and-continue)
use thiserror::Error; // For domain errors (position violations, risk breaches)

// ✅ API call: Use anyhow::Result (may fail, but retry or skip)
async fn fetch_account_info() -> Result<AccountInfo> {
    let resp = client.get_account().await?;  // Returns Err, not panic
    Ok(resp)
}

// ✅ Risk violation: Use thiserror (fatal to this trade, must block)
#[derive(Error, Debug)]
pub enum RiskViolation {
    #[error("Position size {0} exceeds limit {1}")]
    PositionTooLarge(Decimal, Decimal),
    
    #[error("Daily loss limit exceeded: ${0}")]
    DailyLossLimit(Decimal),
}

// Always log at boundaries (WebSocket events, order fills):
match executor.execute_signal(signal).await {
    Ok(result) => info!("Trade executed: {}", result.order_id),
    Err(e) => error!("Execution failed: {}", e),  // Don't panic, log and continue
}
```

### 7. **Backtesting via Event Replay**

`BacktestEngine` in `src/backtest/engine.rs` replays historical trades:

```rust
// Create engine with config
let config = BacktestConfig {
    symbol: "BTCUSDT".into(),
    initial_capital: dec!(10000),
    position_size: dec!(1000),
    take_profit_bps: dec!(10),
    stop_loss_bps: dec!(5),
    max_hold_time_ms: 5000,
    slippage_bps: dec!(2),     // Assume 2 bps slippage
    commission_bps: dec!(4),   // Binance futures fee
    latency_ms: 100,           // Assume 100ms round-trip
};

let mut engine = BacktestEngine::new(config);

// Feed historical events (trades, depths)
for event in historical_events {
    engine.process_event(event)?;
}

// Get results
let results = engine.finalize();
println!("Total Return: {:.2}%", results.total_return_percent);
println!("Sharpe Ratio: {:.2}", results.sharpe_ratio);
println!("Win Rate: {:.1}%", results.win_rate);
```

Run via: `cargo run --release --bin backtester -- --start 2024-01-01 --end 2024-12-31`

---

## Integration Points & External Dependency Details

### Binance Futures API

**WebSocket (Real-Time Data):**
- Endpoint: `wss://fstream.binance.com/ws`
- Streams: `<symbol>@depth@100ms`, `<symbol>@aggTrade`
- Handler: `BinanceWebSocket` (tokio-tungstenite + futures)
- Features: Auto-reconnect with exponential backoff, message parsing

**REST API (Order Execution):**
- Endpoint: `https://fapi.binance.com/fapi/v1/`
- Authentication: HMAC-SHA256 signatures (see `src/exchange/binance/auth.rs`)
- Timeouts: 5s order placement, 10s account queries
- Handler: `BinanceRestClient` (reqwest + JSON)

**Order Book Sync:**
1. Get full snapshot from REST `/depth?limit=1000`
2. Stream WebSocket depth deltas
3. Apply deltas in-order using update timestamps
4. Skip updates older than snapshot timestamp

**API Keys:**
- Required: `BINANCE_API_KEY`, `BINANCE_SECRET_KEY` in `.env`
- Used by: `BinanceRestClient::new()` at startup
- Never log or expose keys

**Error Handling:**
- Network timeouts: Retry with exponential backoff
- 429 (rate limited): Respect X-MBX-Used-Weight header
- 1000 (order rejected): Log and skip, don't panic
- WebSocket disconnect: Auto-reconnect, replay missed depth

### Market Data Types

```rust
pub enum MarketEvent {
    Depth {
        bids: Vec<(Decimal, Decimal)>,
        asks: Vec<(Decimal, Decimal)>,
        timestamp: SystemTime,
    },
    Trade {
        id: u64,
        price: Decimal,
        qty: Decimal,
        is_buyer_maker: bool,
    },
    DepthSnapshot { ... },
}
```

---

## File Responsibility Matrix

**Core Trading Logic:**
- `src/data/orderbook.rs` - Order book state (bids/asks), depth calculations
- `src/strategy/signals/imbalance.rs` - Volume imbalance detection (Z-score)
- `src/strategy/signals/flow.rs` - Aggressive order flow (clustering, momentum)
- `src/strategy/execution.rs` - Position sizing, entry/exit logic, P&L tracking
- `src/risk/limits.rs` - Circuit breaker, position limits, drawdown checks
- `src/risk/position.rs` - Individual position PnL, holding duration

**Exchange Integration:**
- `src/exchange/binance/websocket.rs` - Real-time depth/trade streaming
- `src/exchange/binance/rest.rs` - Order placement, account queries
- `src/exchange/binance/auth.rs` - HMAC-SHA256 signature generation
- `src/exchange/binance/types.rs` - Binance API request/response types

**Configuration & Utilities:**
- `src/utils/config.rs` - TOML parsing, environment modes
- `src/utils/logger.rs` - Tracing setup, metrics collection

**Backtesting:**
- `src/backtest/engine.rs` - Event replay, fill simulation, metrics

**Entry Points (pick one per run):**
- `src/bin/paper_trader.rs` - Paper trading mode (RECOMMENDED START)
- `src/bin/live_trader.rs` - Real money (⚠️ requires review of code + credentials)
- `src/bin/backtester.rs` - Historical backtesting
- `src/bin/data_collector.rs` - Utility to collect historical data

**Reference:**
- `GREYPAPER.md` - Technical specification (market theory, math, strategy details)
- `README.md` - Quick start, Codespaces setup
- `Makefile` - All development commands

---

## Common Development Patterns

### Add a New Signal Detector

1. Create `src/strategy/signals/mynewsignal.rs`:
```rust
pub struct MySignalDetector {
    history: VecDeque<f64>,
    threshold: f64,
}

impl MySignalDetector {
    pub fn new(threshold: f64) -> Self { ... }
    
    pub fn calculate_signal(&mut self, orderbook: &OrderBook) -> Option<Signal> {
        // Return Some(Signal) if condition met, None otherwise
    }
}
```

2. Add to `src/strategy/signals/mod.rs`:
```rust
pub mod mynewsignal;
pub use mynewsignal::MySignalDetector;
```

3. Add variant to `CompositeSignal` and update voting logic in `composite.rs`

### Tune Strategy Parameters

**Never hardcode parameters.** Always use config files:

```toml
# config/paper_trading.toml
[strategy]
imbalance_threshold = 3.0      # Adjust this, not the code
min_confirming_signals = 2
lookback_window_ms = 5000
take_profit_bps = 10
stop_loss_bps = 5
```

Load via `Config::load()`:
```rust
let config = Config::load()?;
let threshold = config.strategy.imbalance_threshold;  // 3.0
```

### Add a Risk Limit

Edit `src/risk/limits.rs`:
```rust
pub struct RiskLimits {
    pub max_position_size: Decimal,
    pub max_daily_loss: Decimal,
    pub new_limit: Decimal,  // Add here
}

// Add check in RiskManager::can_open_position():
if position_size > limits.new_limit {
    return Err(RiskViolation { ... });
}
```

### Test with Backtester

```bash
cargo run --release --bin backtester \
    --start 2024-01-01 \
    --end 2024-12-31 \
    --symbol BTCUSDT \
    --capital 10000
```

Review output metrics (total return, Sharpe, win rate, max drawdown).

---

## Performance Considerations

**Hot Paths (optimize here):**
- Order book updates: Target <1ms via DashMap lock-free reads
- Signal calculation: Target <5ms per update (aggregate 3 detectors)
- Total latency: 50-100ms (network + processing)

**Optimizations in Cargo.toml:**
```toml
[profile.release]
opt-level = 3           # -O3 optimization
lto = true             # Link-time optimization
codegen-units = 1      # Better optimization, slower compile
panic = "abort"        # Slightly faster, no unwinding
```

**Code-level optimizations:**
- `#[inline]` frequently-called small methods (e.g., `update_level`)
- Pre-allocate `Vec` and `VecDeque` with capacity
- Avoid allocations in hot loops (use update-in-place)
- Batch updates when possible
- Use `&mut self` for mutable operations

**Benchmarking:**
```bash
make bench  # Generates HTML report in target/criterion/
```

See `benches/orderbook_bench.rs` for examples.

---

## Project Dependencies Summary

| Crate | Version | Purpose | Hot Path? |
|-------|---------|---------|-----------|
| `tokio` | 1.35 | Async runtime | YES |
| `tokio-tungstenite` | 0.21 | WebSocket | YES |
| `reqwest` | 0.11 | REST client | NO |
| `rust_decimal` | 1.33 | Financial math | YES |
| `dashmap` | 5.5 | Lock-free order book | YES |
| `crossbeam` | 0.8 | Channels, lock-free | YES |
| `serde/serde_json` | 1.0 | JSON parsing | NO |
| `tracing` | 0.1 | Structured logging | NO |
| `prometheus` | 0.13 | Metrics | NO |
| `chrono` | 0.4 | Timestamps | YES |
| `hmac/sha2` | 0.12/0.10 | Auth signatures | NO |
| `clap` | 4.4 | CLI args | NO |

---

## Getting Started (Next Steps)

1. **Read GREYPAPER.md** (900 lines) - Market theory, signal math, strategy details
2. **Run paper_trader:** `make paper` - See system in action with live data, no risk
3. **Backtest:** `make backtest` - Validate strategy on historical data
4. **Review code:** Start with `src/lib.rs` re-exports, then follow data flow
5. **Modify parameters:** Only edit config files, never hardcode
6. **For live trading:** Review every security aspect, test again on paper, know the risks

---

**⚠️ WARNING:** This system trades REAL MONEY on live. Only use capital you can afford to lose. Do not enable live trading without thorough backtesting and understanding all code.
