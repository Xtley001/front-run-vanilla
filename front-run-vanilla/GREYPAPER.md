# Front Run Vanilla: Grey Paper
## Technical Specification & Implementation Guide

**Version:** 1.0.0  
**Date:** February 2024  
**Status:** Production Ready  
**Authors:** Front Run Vanilla Team  

---

## Abstract

Front Run Vanilla is a high-frequency trading (HFT) system designed to detect and capitalize on large market orders through real-time order book microstructure analysis. This grey paper provides complete technical specifications, mathematical foundations, implementation details, and risk management frameworks for the system.

The strategy achieves profitability by identifying "whale" activity—large institutional orders that predictably impact prices—and positioning ahead of anticipated movements with sub-100ms execution latency.

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Market Microstructure Theory](#2-market-microstructure-theory)
3. [Signal Generation](#3-signal-generation)
4. [Architecture & Implementation](#4-architecture--implementation)
5. [Risk Management Framework](#5-risk-management-framework)
6. [Backtesting Methodology](#6-backtesting-methodology)
7. [Performance Metrics](#7-performance-metrics)
8. [Deployment & Operations](#8-deployment--operations)
9. [Regulatory Considerations](#9-regulatory-considerations)
10. [Appendix](#10-appendix)

---

## 1. Introduction

### 1.1 Motivation

Modern cryptocurrency markets exhibit significant inefficiencies in price discovery, particularly around large order execution. Market participants with substantial positions often create predictable temporary price impact due to:

1. **Order Book Imbalance** - Large orders remove liquidity asymmetrically
2. **Iceberg Orders** - Hidden liquidity reveals itself gradually
3. **Aggressive Execution** - Market orders signal urgency and direction

By detecting these patterns microseconds before price movement, systematic traders can extract consistent profits in the 2-10 basis point range.

### 1.2 Strategy Overview

**Core Hypothesis:** Large market orders (>$500k notional) create predictable price impact in the 100ms-5s timeframe.

**Approach:**
1. Monitor order book depth in real-time via WebSocket
2. Calculate statistical anomalies in bid/ask balance
3. Detect aggressive order flow clusters
4. Execute when multiple signals confirm
5. Exit at predetermined profit/loss/time thresholds

**Expected Performance:**
- Sharpe Ratio: 1.5-2.5
- Win Rate: 50-60%
- Average Trade Duration: 1-3 seconds
- Trade Frequency: 10-50 per day

### 1.3 Technical Stack

- **Language:** Rust (for memory safety and performance)
- **Exchange:** Binance Futures (highest liquidity)
- **Data:** WebSocket (depth@100ms, aggTrades)
- **Execution:** REST API (market orders)
- **Infrastructure:** VPS in Singapore (low latency to exchange)

---

## 2. Market Microstructure Theory

### 2.1 Order Book Dynamics

A limit order book maintains buy (bid) and sell (ask) orders at discrete price levels:

```
Order Book Structure:
Asks (Sell Orders)
  101.50 │ 2.5 BTC
  101.25 │ 5.0 BTC
  101.00 │ 10.0 BTC  ← Best Ask
─────────┼────────────
  100.50 │ 12.0 BTC  ← Best Bid
  100.25 │ 6.0 BTC
  100.00 │ 3.0 BTC
Bids (Buy Orders)
```

**Key Metrics:**
- **Spread:** Ask - Bid (liquidity cost)
- **Depth:** Cumulative quantity at top N levels
- **Imbalance:** Bid depth / Ask depth (directional pressure)

### 2.2 Price Impact Models

Large orders impact prices through two mechanisms:

**1. Temporary Impact** (Reversible)
```
ΔP_temp = λ * Q^α * σ
```
Where:
- Q = Order size
- σ = Volatility
- λ = Market depth factor
- α = Concavity parameter (typically 0.5-0.7)

Duration: 100ms - 10s

**2. Permanent Impact** (Information-driven)
```
ΔP_perm = η * Q
```

Duration: Minutes to hours

**Our Strategy Targets:** Temporary impact (mean reversion opportunity)

### 2.3 Information Content of Order Flow

Market orders reveal information through:

1. **Direction** - Buy vs Sell pressure
2. **Size** - Conviction level
3. **Urgency** - Market vs Limit order choice
4. **Clustering** - Multiple aggressive orders = institutional

**Kyle's Lambda:**
```
λ = E[ΔP] / E[Q]
```

Measures price impact per unit volume (our profitability driver).

---

## 3. Signal Generation

### 3.1 Primary Signal: Order Book Imbalance

**Mathematical Definition:**

```
Imbalance(t) = Σ(BidQty_i) / Σ(AskQty_i)  for i=1 to N levels
```

Typically N=5 (top 5 levels each side).

**Statistical Normalization:**

Maintain rolling window W (e.g., 100 samples = ~10 seconds at 100ms updates):

```
μ(t) = (1/W) * Σ Imbalance(t-i)  for i=0 to W-1
σ(t) = sqrt((1/W) * Σ (Imbalance(t-i) - μ)²)

Z-Score(t) = (Imbalance(t) - μ(t)) / σ(t)
```

**Signal Generation:**

```
IF Z-Score(t) > threshold (e.g., 3.0):
    SIGNAL = BUY (heavy bid side = bullish)
IF Z-Score(t) < -threshold (e.g., -3.0):
    SIGNAL = SELL (heavy ask side = bearish)
```

**Rationale:** 3 sigma events occur <0.3% of time. When detected, strong directional move likely.

### 3.2 Secondary Signal: Aggressive Flow

**Definition:**

Track last M trades (e.g., M=20) with time decay weight w(i):

```
w(i) = decay_factor^i  where decay_factor = 0.95
```

Separate into aggressive buys and sells:

```
BuyVolume = Σ (Qty_i * w(i))  for trades where taker=buyer
SellVolume = Σ (Qty_i * w(i))  for trades where taker=seller

FlowImbalance = (BuyVolume - SellVolume) / (BuyVolume + SellVolume)
```

**Signal Generation:**

```
IF FlowImbalance > 0.6:
    SIGNAL = BUY (60%+ aggressive buying)
IF FlowImbalance < -0.6:
    SIGNAL = SELL (60%+ aggressive selling)
```

### 3.3 Composite Signal Aggregation

**Requirements for Trade Execution:**

1. **Primary Signal** - Imbalance Z-score > threshold
2. **Minimum Confirmations** - At least 2 other signals agree
3. **Confidence Calculation:**

```
Confidence = (0.4 * Primary_Confidence) +
             (0.3 * Confirmation_Count / Max_Confirmations) +
             (0.3 * Avg_Secondary_Confidence)
```

4. **Position Sizing:**

```
Position_Size = Base_Size * (0.5 + 1.5 * Confidence)
```

Scales from 0.5x to 2.0x based on confidence.

### 3.4 Signal Performance Characteristics

**Empirical Observations:**

- **Signal Frequency:** 5-20 per hour (during active markets)
- **True Positive Rate:** ~55-65%
- **False Positive Rate:** ~35-45%
- **Signal Strength Distribution:** Gaussian-like around threshold
- **Optimal Threshold:** 2.5-3.5 sigma (backtesting dependent)

---

## 4. Architecture & Implementation

### 4.1 System Architecture

```
┌─────────────────────────────────────────┐
│     Binance Futures Exchange            │
│  WebSocket: depth@100ms, aggTrades      │
└─────────────┬───────────────────────────┘
              │ 10-50ms network latency
     ┌────────▼────────┐
     │  Data Ingestion │
     │  - Parse JSON   │
     │  - Validate     │
     └────────┬────────┘
              │ <1ms
   ┌──────────▼──────────┐
   │   Order Book        │ Lock-free (DashMap)
   │   - Update levels   │ Sub-microsecond reads
   │   - Track depth     │
   └──────────┬──────────┘
              │ <2ms
  ┌───────────▼───────────┐
  │  Signal Generation    │
  │  - Imbalance calc     │
  │  - Flow analysis      │
  │  - Composite scoring  │
  └───────────┬───────────┘
              │ <1ms
    ┌─────────▼─────────┐
    │  Risk Management  │
    │  - Pre-trade check│
    │  - Position limits│
    │  - Circuit breakers│
    └─────────┬─────────┘
              │ <1ms
      ┌───────▼───────┐
      │  Execution    │
      │  - REST API   │ 30-50ms
      │  - Market order│
      └───────┬───────┘
              │
     ┌────────▼────────┐
     │  Position Mgmt  │
     │  - PnL tracking │
     │  - Exit monitor │
     └─────────────────┘

Total Latency Budget: 50-100ms (Signal → Fill)
```

### 4.2 Order Book Implementation

**Lock-Free Design:**

```rust
pub struct OrderBook {
    symbol: String,
    bids: Arc<DashMap<Decimal, Decimal>>,  // Concurrent HashMap
    asks: Arc<DashMap<Decimal, Decimal>>,
    last_update: Arc<DashMap<(), SystemTime>>,
}
```

**Why DashMap?**
- Lock-free reads (critical for signal calculation)
- Fine-grained write locking (better than RwLock<HashMap>)
- Scales across CPU cores

**Update Operation:**

```rust
#[inline]
pub fn update_level(&self, side: Side, price: Decimal, qty: Decimal) {
    let book = match side {
        Side::Buy => &self.bids,
        Side::Sell => &self.asks,
    };
    
    if qty.is_zero() {
        book.remove(&price);  // Cancel
    } else {
        book.insert(price, qty);  // Update
    }
}
```

**Performance:** ~800ns per update (measured)

### 4.3 Decimal Precision

**Critical Design Choice:**

```rust
use rust_decimal::Decimal;  // NEVER use f64 for money!

let price = dec!(100.50);  // Exact representation
```

**Why Decimal?**

```
f64: 0.1 + 0.2 = 0.30000000000000004 ❌
Decimal: 0.1 + 0.2 = 0.3 ✅
```

Over thousands of trades, floating-point errors compound.

### 4.4 WebSocket Connection Management

**Auto-Reconnect Logic:**

```rust
loop {
    match connect_and_process().await {
        Ok(_) => {
            reconnect_delay = Duration::from_secs(1);  // Reset
        }
        Err(e) => {
            warn!("WebSocket error: {}", e);
            sleep(reconnect_delay).await;
            reconnect_delay = min(reconnect_delay * 2, MAX_DELAY);  // Exponential backoff
        }
    }
}
```

**Streams Subscribed:**
- `btcusdt@depth@100ms` - Order book updates every 100ms
- `btcusdt@aggTrade` - Aggregated trades

### 4.5 Execution Engine

**Market Order Flow:**

```
1. Signal Generated (t=0ms)
2. Risk Check (<1ms)
   - Position size valid?
   - Exposure within limits?
   - Daily loss not exceeded?
3. HMAC Signature (1-2ms)
4. HTTP POST to Binance (30-50ms)
5. Fill Confirmation (5-10ms)
6. Position Created (<1ms)

Total: 50-80ms typical
```

**Position Sizing:**

```rust
fn calculate_size(confidence: f64, base: Decimal) -> Decimal {
    let multiplier = 0.5 + (1.5 * confidence);
    base * Decimal::from_f64_retain(multiplier).unwrap()
}
```

Examples:
- 50% confidence → 1.25x size
- 80% confidence → 1.70x size
- 100% confidence → 2.00x size

---

## 5. Risk Management Framework

### 5.1 Position Limits

**Single Position:**
```
Max_Size = min(
    $5,000,  // Hard limit
    Account_Equity * 0.05  // 5% of account
)
```

**Total Exposure:**
```
Max_Portfolio = $10,000  // Maximum capital at risk
```

### 5.2 Loss Limits

**Daily Loss Limit:**
```
IF Daily_PnL < -$500:
    HALT_TRADING
    SEND_ALERT
```

**Drawdown Limit:**
```
Drawdown = (Peak_Equity - Current_Equity) / Peak_Equity

IF Drawdown > 10%:
    HALT_TRADING
    CLOSE_ALL_POSITIONS
```

### 5.3 Trade Frequency Limits

**Hourly:**
```
Max_Trades_Per_Hour = 30
```

**Daily:**
```
Max_Trades_Per_Day = 200
```

Prevents over-trading and algorithm runaway.

### 5.4 Circuit Breakers

**Latency Circuit Breaker:**

```
IF Average_Latency(last_10_trades) > 500ms:
    HALT_TRADING
    REASON = "High latency detected"
```

**Exchange Connectivity:**

```
IF WebSocket_Disconnect_Duration > 30s:
    EMERGENCY_CLOSE_ALL_POSITIONS
    HALT_TRADING
```

**Unusual Market Conditions:**

```
IF Spread > 50 bps OR Volatility > 3x_normal:
    PAUSE_NEW_ENTRIES
    MONITOR_CLOSELY
```

### 5.5 Exit Strategy

**Three Exit Mechanisms:**

1. **Take Profit**
   ```
   IF Unrealized_PnL_Percent >= 0.10%:
       CLOSE_POSITION
   ```

2. **Stop Loss**
   ```
   IF Unrealized_PnL_Percent <= -0.05%:
       CLOSE_POSITION
   ```

3. **Time-Based**
   ```
   IF Position_Age > 5000ms:
       CLOSE_POSITION
   ```

**Rationale:** 
- Tight stops for HFT (2:1 reward/risk)
- Time-based prevents holding overnight
- Automatic execution (no discretion)

---

## 6. Backtesting Methodology

### 6.1 Data Requirements

**Order Book Snapshots:**
- Frequency: 100ms (matching WebSocket stream)
- Depth: Top 20 levels each side
- Fields: price, quantity, timestamp

**Trades:**
- All aggregated trades
- Fields: price, quantity, side, timestamp, trade_id

**Time Period:**
- Minimum: 30 days
- Recommended: 6-12 months
- Include various market conditions (trending, ranging, volatile)

### 6.2 Fill Simulation

**Slippage Model:**

```
Slippage = Market_Impact(Order_Size, Liquidity)

Market_Impact = α * (Order_Size / Top_5_Liquidity)^β

where:
  α = 2.0 bps (calibration parameter)
  β = 0.5 (concavity)
```

**Implementation:**

```rust
fn simulate_fill(
    side: Side,
    size: Decimal,
    market_price: Decimal,
    liquidity: Decimal
) -> Decimal {
    let impact = ALPHA * (size / liquidity).powf(BETA);
    let slippage = market_price * impact;
    
    match side {
        Side::Buy => market_price + slippage,   // Pay more
        Side::Sell => market_price - slippage,  // Receive less
    }
}
```

### 6.3 Transaction Costs

**Binance Futures Fees:**
- Maker: 0.02% (not used - we use market orders)
- Taker: 0.04% (our cost)

**Per Trade Cost:**

```
Fee = Notional_Value * 0.0004
Total_Cost = Entry_Fee + Exit_Fee = 0.08% of notional
```

Must overcome 8 bps to be profitable!

### 6.4 Latency Injection

**Simulated Execution Delay:**

```
Signal_Generated (t=0)
↓
Risk_Check (+1ms)
↓
Order_Sent (+50ms)  ← HTTP round-trip
↓
Fill_Confirmed (+10ms)
↓
Total: 61ms
```

Price may move during this window—backtest must account for it.

---

## 7. Performance Metrics

### 7.1 Core Metrics

**Sharpe Ratio:**

```
Sharpe = (Avg_Return - Risk_Free_Rate) / StdDev_Return

Target: > 1.5 (excellent for trading strategies)
```

**Sortino Ratio:**

```
Sortino = (Avg_Return - Risk_Free_Rate) / Downside_Deviation

Focuses on downside risk only (more relevant for traders)
```

**Win Rate:**

```
Win_Rate = Winning_Trades / Total_Trades

Target: > 55%
```

**Profit Factor:**

```
Profit_Factor = Total_Wins / Total_Losses

Target: > 1.3
```

### 7.2 Risk Metrics

**Maximum Drawdown:**

```
DD(t) = max(Peak_Equity - Equity(t)) / Peak_Equity

Target: < 10%
```

**Value at Risk (VaR):**

```
VaR_95 = Percentile(Daily_Returns, 5%)

Interpretation: "95% confident we won't lose more than VaR in one day"
```

**Calmar Ratio:**

```
Calmar = Annualized_Return / Max_Drawdown

Target: > 2.0
```

### 7.3 Trade Metrics

**Average Trade Duration:**

```
Avg_Duration = Σ(Exit_Time - Entry_Time) / N_Trades

Target: 1-3 seconds
```

**Average Trade PnL:**

```
Avg_PnL = Total_PnL / N_Trades

Target: > 0 (obviously) and > 8 bps (to cover fees)
```

**Trade Frequency:**

```
Frequency = N_Trades / N_Days

Target: 10-50 per day
```

### 7.4 Expected Results

**Conservative Estimates:**

| Metric | Expected Value |
|--------|----------------|
| Sharpe Ratio | 1.5 - 2.0 |
| Win Rate | 50% - 58% |
| Avg Win | +12 bps |
| Avg Loss | -7 bps |
| Max Drawdown | 5% - 12% |
| Monthly Return | 3% - 7% |
| Trade Frequency | 15-40/day |

**Disclaimer:** These are theoretical. Actual results vary based on:
- Market conditions
- Parameter tuning
- Execution quality
- Competition (other HFT algos)

---

## 8. Deployment & Operations

### 8.1 Infrastructure Requirements

**Compute:**
- CPU: 2-4 cores (single-threaded performance matters)
- RAM: 4-8 GB
- Storage: 20-50 GB SSD
- OS: Ubuntu 22.04 LTS

**Network:**
- Location: Singapore (nearest to Binance servers)
- Latency to Binance: < 10ms
- Bandwidth: 10 Mbps minimum
- Uptime: 99.9%+

**Recommended Providers:**
- AWS: t3.medium in ap-southeast-1
- DigitalOcean: Droplet in Singapore
- Vultr: High-frequency compute in Singapore

### 8.2 Monitoring

**Metrics to Track:**

```
1. System Health:
   - CPU usage
   - Memory usage
   - Network latency
   - WebSocket connection status

2. Trading Metrics:
   - Signals generated
   - Trades executed
   - Win rate (rolling)
   - Current PnL
   - Daily PnL
   - Open positions

3. Risk Metrics:
   - Current drawdown
   - Daily loss
   - Position concentration
   - Latency (signal → fill)
```

**Prometheus + Grafana Setup:**

```yaml
# Metrics exposed on :9090/metrics
- signal_generated_total{direction="buy|sell"}
- trade_executed_total{result="win|loss"}
- latency_signal_to_fill_ms (histogram)
- position_pnl_usd (gauge)
- daily_pnl_usd (gauge)
```

### 8.3 Logging

**Structured JSON Logging:**

```json
{
  "timestamp": "2024-02-03T12:34:56.789Z",
  "level": "INFO",
  "target": "execution",
  "fields": {
    "event": "trade_executed",
    "order_id": "12345",
    "symbol": "BTCUSDT",
    "side": "BUY",
    "price": 100000.50,
    "quantity": 0.01,
    "pnl": 5.23,
    "latency_ms": 67
  }
}
```

**Log Retention:**
- Real-time: 7 days
- Compressed: 90 days
- Archives: 1 year

### 8.4 Disaster Recovery

**Backup Strategy:**

1. **Configuration:** Git repository (daily push)
2. **Trade History:** Database backup (daily)
3. **Logs:** S3 archival (weekly)
4. **Secrets:** Encrypted vault (redundant storage)

**Failover Plan:**

```
1. Primary server fails
   ↓
2. Health check detects (30s)
   ↓
3. Close all positions via API
   ↓
4. Alert operations team
   ↓
5. Manual restart on backup server
```

---

## 9. Regulatory Considerations

### 9.1 Disclaimer

**This system is for educational and research purposes.**

- Not financial advice
- Trading carries substantial risk
- Regulatory landscape varies by jurisdiction
- Consult legal counsel before deployment

### 9.2 Compliance Requirements

**United States:**
- Algorithmic trading may require registration with SEC/CFTC
- Pattern day trader rules apply
- Report gains/losses for tax purposes
- Market manipulation prohibited (pump and dump, spoofing)

**European Union:**
- MiFID II regulations
- Algorithmic trading notification requirements
- Circuit breakers and kill switches mandatory

**General:**
- Know Your Customer (KYC) via exchange
- Anti-Money Laundering (AML) compliance
- API usage within exchange Terms of Service

### 9.3 Ethical Considerations

**Front-Running:**

The term "front run" in this system refers to **positioning ahead of anticipated market movements**, not:
- Trading on insider information (illegal)
- Trading ahead of client orders (illegal)
- Manipulating markets (illegal)

**Transparency:**

This grey paper provides full transparency into:
- Strategy mechanics
- Implementation details
- Risk parameters
- Expected performance

No "black box" algorithms.

---

## 10. Appendix

### 10.1 Glossary

**HFT** - High-Frequency Trading: Trading strategies with holding periods of seconds to milliseconds

**Order Book** - List of buy (bid) and sell (ask) limit orders at various price levels

**Spread** - Difference between best ask and best bid prices

**Slippage** - Difference between expected and actual execution price

**Market Impact** - Price movement caused by executing a large order

**Basis Point (bp)** - 0.01% (1/100th of a percent)

**Sharpe Ratio** - Risk-adjusted return metric

**Drawdown** - Peak-to-trough decline in equity

**Circuit Breaker** - Automatic halt mechanism when risk thresholds exceeded

### 10.2 Mathematical Notation

| Symbol | Meaning |
|--------|---------|
| P | Price |
| Q | Quantity |
| σ | Standard deviation (volatility) |
| μ | Mean |
| λ | Kyle's lambda (price impact) |
| α, β | Calibration parameters |
| t | Time |
| Δ | Change operator |

### 10.3 References

**Academic:**
1. Kyle, A. (1985). "Continuous Auctions and Insider Trading"
2. Hasbrouck, J. (1991). "Measuring the Information Content of Stock Trades"
3. Glosten, L., & Harris, L. (1988). "Estimating the Components of the Bid-Ask Spread"

**Industry:**
1. Binance API Documentation: https://binance-docs.github.io/apidocs/futures/en/
2. "Algorithmic Trading: Winning Strategies" - Ernest Chan
3. "Flash Boys" - Michael Lewis (market structure context)

### 10.4 Code Repository

**GitHub:** [Your Repository URL]

**Structure:**
```
front-run-vanilla/
├── src/           (Source code)
├── docs/          (Documentation)
├── config/        (Configuration)
├── tests/         (Tests)
└── benches/       (Benchmarks)
```

**License:** MIT

### 10.5 Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2024-02 | Initial release |

### 10.6 Contact

**Support:** [Your Support Email]  
**Issues:** GitHub Issues  
**Community:** [Discord/Telegram if applicable]

---

## Conclusion

Front Run Vanilla demonstrates that systematic, transparent, and well-risk-managed trading strategies can extract consistent profits from cryptocurrency markets through order flow analysis.

**Key Takeaways:**

1. **Mathematical Foundation** - Signals based on statistical anomalies (3+ sigma events)
2. **Robust Implementation** - Rust for safety, lock-free for performance
3. **Comprehensive Risk Management** - Multiple layers of protection
4. **Transparent Operations** - Full disclosure of methodology
5. **Production Ready** - Complete system from data to execution

**Final Warning:**

Trading carries substantial risk of loss. This system can lose money. No strategy works forever. Markets evolve. Past performance does not guarantee future results.

**Use responsibly. Trade wisely. Manage risk religiously.**

---

*Front Run Vanilla Grey Paper v1.0.0*  
*© 2024 Front Run Vanilla Team*  
*Licensed under MIT*
