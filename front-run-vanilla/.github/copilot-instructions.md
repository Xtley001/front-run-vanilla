# Copilot Instructions for Front Run Vanilla

## Project Overview

**Front Run Vanilla** is a production-grade high-frequency trading (HFT) system for cryptocurrency markets. It detects large market orders through real-time order book analysis and executes trades with <100ms latency. Built in Rust for the Binance Futures exchange.

Core strategy: Identify "whale" activity (>$500k orders) via statistical anomalies in bid/ask imbalance and order flow, then capitalize on predictable price impact (2-10 basis points per trade, 10-50 trades/day).

## Critical Architecture Pattern

**Data Flow Pipeline:** `Binance WebSocket → OrderBook → Signals → RiskCheck → Execution`

The system uses lock-free concurrent data structures (`DashMap`) for sub-millisecond order book updates, then chains three independent signal detectors through `SignalAggregator` for composite signals before risk evaluation.

**Key Components:**

- `[src/data/orderbook.rs](src/data/orderbook.rs)` - Lock-free concurrent order book using `DashMap`; tracks bid/ask depth updates from WebSocket with <1ms latency
- `[src/strategy/signals/](src/strategy/signals/)` - Three signal generators: `ImbalanceDetector` (bid/ask volume ratios), `FlowAnalyzer` (aggressive order clustering), and `CompositeSignal` (voting mechanism)
- `[src/risk/limits.rs](src/risk/limits.rs)` - Risk enforcement: position size, daily loss, portfolio exposure thresholds
- `[src/backtest/engine.rs](src/backtest/engine.rs)` - Replay engine for historical validation

## Essential Developer Workflows

**Build & Test:**
```bash
make build       # Release build (optimized)
make test        # Run all tests with release optimization
make bench       # Performance benchmarks
make check       # Clippy lints
```

**Trading Modes:**
```bash
make paper       # Paper trading (no real money, config: paper_trading.toml)
make backtest    # Historical backtesting (config: backtest.toml)
make live        # Real trading (⚠️ DANGER: production.toml, requires API keys)
```

**Development:**
```bash
make format      # Format code
make watch       # Auto-rebuild on changes
make lint        # Full linting suite
```

## Project-Specific Conventions

### 1. **Fixed-Point Arithmetic for Financial Values**

CRITICAL: Use `Decimal` from `rust_decimal` crate for ALL prices, quantities, and monetary amounts. Never use `f64` for financial calculations—this prevents floating-point precision loss that compounds in high-frequency trading.

```rust
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// ✅ Correct
let price = Decimal::from_str("100.50")?;
let quantity = dec!(10.5);

// ❌ NEVER do this for money
let price = 100.50_f64;
```

### 2. **Configuration via TOML with Environment-Specific Overrides**

All configuration lives in `[config/](config/)` with three modes:
- `backtest.toml` - Historical testing (aggressive position sizing)
- `paper_trading.toml` - Live data, simulated fills (conservative parameters)
- `production.toml` - Real money (tightest risk limits)

Load via `Config::from_file()` in `[src/utils/config.rs](src/utils/config.rs)`. Never hardcode parameters.

### 3. **Async/Await with Tokio, Lock-Free Concurrency**

- Use `tokio::spawn()` for WebSocket listener, risk checker, and execution engine tasks
- Share data via `Arc<DashMap<K, V>>` (NOT `Mutex`) for order book—enables concurrent reads without blocking
- Use `crossbeam::channel` for task communication (bounded `MPSC` preferred)
- WebSocket: `tokio-tungstenite` with `futures-util` stream combinators

### 4. **Structured Logging & Metrics**

Use `tracing` crate with JSON-structured output for production debugging:

```rust
tracing::info!(symbol = "BTCUSDT", imbalance = %imbalance, signal_count = 3, "signal generated");
```

Prometheus metrics exported in binary targets for monitoring (see `[src/utils/logger.rs](src/utils/logger.rs)`).

### 5. **Signal Composition via Voting**

`CompositeSignal` requires configurable minimum confirming signals (typically 2-3 of 3):
- `ImbalanceDetector`: Volume asymmetry ratio
- `FlowAnalyzer`: Aggressive order clustering and momentum
- Additional signals can be added—all vote via enum variants

### 6. **Error Handling Strategy**

Use `anyhow::Result<T>` for recoverable errors (API calls, config loading), `thiserror::Error` for domain types (position violations, risk breaches). Propagate with `?` operator; log at boundaries (WebSocket events, order fills).

### 7. **Backtesting via Event Replay**

`BacktestEngine` replays CSV trade history, simulating fills based on `SimulatedFill` heuristics. Stored in `[src/backtest/engine.rs](src/backtest/engine.rs)`. Run via `cargo run --release --bin backtester`.

## Integration Points & External Dependencies

- **Binance API**: WebSocket depth@100ms stream + REST market orders (via `[src/exchange/binance/](src/exchange/binance/)`)
  - Authentication: HMAC-SHA256 with `.env` credentials (see `[src/exchange/binance/auth.rs](src/exchange/binance/auth.rs)`)
  - REST timeouts: 5s for order placement, 10s for account queries
- **Order Book Sync**: First snapshot from REST depth endpoint, then WebSocket deltas
- **Market Events**: Unified `MarketEvent` enum (Depth, Trade, DepthSnapshot)

## Key Files by Responsibility

| File | Purpose |
|------|---------|
| `[Makefile](Makefile)` | All dev commands (make help) |
| `[GREYPAPER.md](GREYPAPER.md)` | Complete technical spec & math |
| `[src/lib.rs](src/lib.rs)` | Module declarations & public re-exports |
| `[config/paper_trading.toml](config/paper_trading.toml)` | Recommended starting config |
| `[src/data/types.rs](src/data/types.rs)` | Core types: Side, Trade, Order, Signal |
| `[src/strategy/execution.rs](src/strategy/execution.rs)` | Trade lifecycle (entry/exit logic) |
| `[src/risk/position.rs](src/risk/position.rs)` | Position tracking & P&L |

## Common Modification Patterns

- **Add new signal**: Create struct in `src/strategy/signals/`, implement trait, add to `CompositeSignal` enum
- **Tune parameters**: Edit config files, not code—strategy reads via `StrategyConfig`
- **Add risk check**: Add method to `RiskManager`, call in execution path
- **Test with history**: Run backtester with event file, validate PnL metrics in `BacktestResults`

## Performance Considerations

- Order book updates: <1ms (DashMap lock-free)
- Signal calculation: <5ms per update
- Total latency target: 50-100ms (includes network)
- WebSocket reconnect: Automatic with exponential backoff (see binance/websocket.rs)
- Avoid allocations in hot loop: Pre-allocate vectors, reuse buffers

---

**Start here for context:** Read [GREYPAPER.md](GREYPAPER.md) (900 lines) for market theory, complete signal math, and risk framework.
