# Front Run Vanilla - HFT Trading System

**Production-Grade Trading System Optimized for GitHub Codespaces**

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Codespaces](https://img.shields.io/badge/Codespaces-Ready-brightgreen.svg)]()

> ⚠️ **WARNING:** This trades REAL MONEY. Can result in financial loss. Only use capital you can afford to lose.

---

## 🚀 Quick Start in GitHub Codespaces

### 1. Open in Codespaces

Click the green "Code" button → "Codespaces" → "Create codespace"

Codespaces will automatically:
- ✅ Install Rust toolchain
- ✅ Build the project
- ✅ Run tests
- ✅ Setup development environment

### 2. Configure API Keys

```bash
# Edit .env file
nano .env

# Add your Binance API credentials
```

### 3. Run Paper Trading (Safe!)

```bash
make paper
# Or: cargo run --release --bin paper_trader
```

---

## 📖 Quick Commands (Makefile)

```bash
make help          # Show all commands
make build         # Build release
make test          # Run tests
make paper         # Paper trading (no real money)
make backtest      # Run backtest
make bench         # Performance benchmarks
```

---

## 📚 Documentation

| Document | Description |
|----------|-------------|
| [GREYPAPER.md](GREYPAPER.md) | **Complete technical specification** |
| [docs/COMPLETE_GUIDE.md](docs/COMPLETE_GUIDE.md) | Setup & usage guide |
| [docs/WEEK_1_SUMMARY.md](docs/WEEK_1_SUMMARY.md) | Foundation details |
| [docs/WEEK_2_SUMMARY.md](docs/WEEK_2_SUMMARY.md) | Signal generation |
| [docs/WEEK_3_SUMMARY.md](docs/WEEK_3_SUMMARY.md) | Execution & risk |

**Start with: [GREYPAPER.md](GREYPAPER.md)** for complete technical details!

---

## 🎯 What This Does

1. **Monitors** Binance order book in real-time
2. **Detects** large orders through statistical analysis
3. **Executes** trades when signals confirm
4. **Manages** risk automatically (stop loss, position limits)
5. **Tracks** performance (PnL, Sharpe ratio, win rate)

**Target:** 2-10 basis points per trade, 10-50 trades/day

---

## 🏗️ Architecture

```
Binance WebSocket → Order Book → Signals → Risk Check → Execute
      ↓                ↓            ↓           ↓          ↓
  Live Data      Lock-Free    Imbalance    Limits    Position
                 (<1ms)       Flow         Checks    Tracking
```

**Performance:**
- Order book: <1ms updates
- Signal calc: <5ms  
- Total latency: 50-100ms

---

## 🧪 Testing in Codespaces

```bash
# Run all tests
make test

# Run benchmarks
make bench

# Check code quality
make lint

# Format code
make format
```

---

## 📊 Expected Performance

| Metric | Range |
|--------|-------|
| Sharpe Ratio | 1.5 - 2.5 |
| Win Rate | 50% - 60% |
| Monthly Return | 3% - 8% |
| Max Drawdown | 5% - 15% |

**Disclaimer:** Past performance ≠ future results

---

## ⚙️ Configuration

Edit `config/production.toml`:

```toml
[strategy]
take_profit_bps = 10.0    # 0.10% profit
stop_loss_bps = 5.0       # 0.05% stop
max_hold_time_ms = 5000   # 5 seconds

[risk]
max_daily_loss_usd = 500  # $500 circuit breaker
```

---

## 🛡️ Safety Features

✅ **Position Limits** - Max $5k per trade  
✅ **Loss Limits** - $500 daily stop  
✅ **Circuit Breakers** - Auto-halt on anomalies  
✅ **Pre-Trade Checks** - 7 checks before every trade  
✅ **Automatic Stops** - Every position has stop loss  

---

## 📦 What's Included

- **29 Rust files** (~6,500 lines production code)
- **Complete documentation** (Grey paper + guides)
- **50+ unit tests** (all passing)
- **Benchmarks** (verify performance)
- **Codespaces config** (instant dev environment)
- **Makefile** (easy commands)

---

## 🚨 Before Live Trading

1. ✅ Run `make test` - all tests pass
2. ✅ Run `make paper` for 7+ days
3. ✅ Run `make backtest` - verify Sharpe >1.5
4. ✅ Use Binance TESTNET first
5. ✅ Start with $100-500 only

---

## 📖 Learn More

- **Technical Details:** See [GREYPAPER.md](GREYPAPER.md)
- **Development:** See [docs/COMPLETE_GUIDE.md](docs/COMPLETE_GUIDE.md)
- **Testing:** Run `make test`
- **Support:** GitHub Issues

---

## 📜 License

MIT License - See [LICENSE](LICENSE)

**Disclaimer:** Educational purposes only. Not financial advice. Trading carries risk of loss.

---

**Ready to code in Codespaces!** 🚀

Press `.` in GitHub to open web editor, or create a Codespace for full development environment.
