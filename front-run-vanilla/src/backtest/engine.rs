use crate::data::{OrderBook, Trade, Side};
use crate::strategy::{ImbalanceDetector, FlowAnalyzer, SignalAggregator, CompositeSignal};
use crate::risk::{Position, PositionManager, RiskManager, RiskLimits};
use rust_decimal::Decimal;
use std::time::{SystemTime, Duration};
use serde::{Serialize, Deserialize};
use anyhow::Result;

/// Backtest configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub symbol: String,
    pub initial_capital: Decimal,
    pub position_size: Decimal,
    pub take_profit_bps: Decimal,
    pub stop_loss_bps: Decimal,
    pub max_hold_time_ms: u64,
    pub slippage_bps: Decimal,
    pub commission_bps: Decimal,
    pub latency_ms: u64,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            symbol: "BTCUSDT".to_string(),
            initial_capital: Decimal::from(10000),
            position_size: Decimal::from(1000),
            take_profit_bps: Decimal::from(10),
            stop_loss_bps: Decimal::from(5),
            max_hold_time_ms: 5000,
            slippage_bps: Decimal::from(2),
            commission_bps: Decimal::from(4),
            latency_ms: 100,
        }
    }
}

/// Market event for backtesting
#[derive(Debug, Clone)]
pub enum BacktestEvent {
    OrderBookUpdate {
        timestamp: SystemTime,
        bids: Vec<(Decimal, Decimal)>,
        asks: Vec<(Decimal, Decimal)>,
    },
    Trade {
        timestamp: SystemTime,
        trade: Trade,
    },
}

/// Simulated fill with slippage
#[derive(Debug, Clone)]
pub struct SimulatedFill {
    pub price: Decimal,
    pub quantity: Decimal,
    pub slippage: Decimal,
    pub commission: Decimal,
}

/// Backtesting engine
pub struct BacktestEngine {
    config: BacktestConfig,
    orderbook: OrderBook,
    position_manager: PositionManager,
    risk_manager: RiskManager,
    
    // Signal generators
    imbalance_detector: ImbalanceDetector,
    flow_analyzer: FlowAnalyzer,
    signal_aggregator: SignalAggregator,
    
    // State tracking
    current_time: SystemTime,
    equity: Decimal,
    equity_curve: Vec<(SystemTime, Decimal)>,
    trades: Vec<BacktestTrade>,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        let orderbook = OrderBook::new(&config.symbol);
        let position_manager = PositionManager::new();
        
        let risk_limits = RiskLimits {
            max_position_size: config.position_size * Decimal::from(5),
            max_portfolio_exposure: config.initial_capital,
            max_daily_loss: config.initial_capital * Decimal::from_f64_retain(0.05).unwrap(),
            max_drawdown_percent: Decimal::from(10),
            max_trades_per_hour: 30,
            max_trades_per_day: 200,
            max_acceptable_latency_ms: 500,
        };
        
        let risk_manager = RiskManager::new(risk_limits, config.initial_capital);
        
        let imbalance_detector = ImbalanceDetector::new(5, 100, 3.0);
        let flow_analyzer = FlowAnalyzer::new(20, 5000, 0.6);
        let signal_aggregator = SignalAggregator::new(3.0, 1.5, 2);

        Self {
            config,
            orderbook,
            position_manager,
            risk_manager,
            imbalance_detector,
            flow_analyzer,
            signal_aggregator,
            current_time: SystemTime::UNIX_EPOCH,
            equity: config.initial_capital,
            equity_curve: vec![],
            trades: vec![],
        }
    }

    /// Process a single market event
    pub fn process_event(&mut self, event: BacktestEvent) -> Result<()> {
        match event {
            BacktestEvent::OrderBookUpdate { timestamp, bids, asks } => {
                self.current_time = timestamp;
                
                // Update order book
                for (price, qty) in bids {
                    self.orderbook.update_level(Side::Buy, price, qty)?;
                }
                for (price, qty) in asks {
                    self.orderbook.update_level(Side::Sell, price, qty)?;
                }

                // Check for signals
                self.check_signals()?;

                // Check exits for open positions
                self.check_exits()?;

                // Record equity
                self.record_equity();
            }
            
            BacktestEvent::Trade { timestamp, trade } => {
                self.current_time = timestamp;
                
                // Process trade for flow analysis
                if let Some(signal) = self.flow_analyzer.process_trade(trade) {
                    self.process_signal(signal)?;
                }
            }
        }

        Ok(())
    }

    /// Check for trading signals
    fn check_signals(&mut self) -> Result<()> {
        let mut signals = Vec::new();

        // Check imbalance
        if let Some(signal) = self.imbalance_detector.calculate_signal(&self.orderbook) {
            signals.push(signal);
        }

        // Aggregate signals
        if let Some(composite) = self.signal_aggregator.aggregate(signals) {
            if composite.is_tradeable(2) {
                self.execute_signal(composite)?;
            }
        }

        Ok(())
    }

    /// Process individual signal
    fn process_signal(&mut self, signal: crate::data::Signal) -> Result<()> {
        // In backtesting, we aggregate all signals before executing
        // This is handled in check_signals()
        Ok(())
    }

    /// Execute a trading signal
    fn execute_signal(&mut self, signal: CompositeSignal) -> Result<()> {
        // Don't trade if already have position
        if self.position_manager.position_count() > 0 {
            return Ok(());
        }

        // Check risk limits
        let position_size = self.config.position_size;
        let current_exposure = self.position_manager.total_exposure();
        
        if let Err(_) = self.risk_manager.can_open_position(position_size, current_exposure) {
            return Ok(()); // Skip trade if risk check fails
        }

        // Get current price
        let current_price = self.orderbook.get_mid_price()
            .ok_or_else(|| anyhow::anyhow!("No mid price available"))?;

        // Simulate fill with slippage and latency
        let fill = self.simulate_fill(signal.direction, current_price, position_size)?;

        // Create position
        let quantity = position_size / fill.price;
        let position = Position::new(
            self.config.symbol.clone(),
            signal.direction,
            fill.price,
            quantity,
            fill.commission,
        );

        self.position_manager.open_position(position)?;

        Ok(())
    }

    /// Check exit conditions
    fn check_exits(&mut self) -> Result<()> {
        let current_price = match self.orderbook.get_mid_price() {
            Some(p) => p,
            None => return Ok(()),
        };

        let positions = self.position_manager.open_positions().to_vec();

        for position in positions {
            let should_exit = 
                position.take_profit_hit(current_price, self.config.take_profit_bps) ||
                position.stop_loss_hit(current_price, self.config.stop_loss_bps) ||
                position.is_expired(self.config.max_hold_time_ms);

            if should_exit {
                self.close_position(&position.symbol, current_price)?;
            }
        }

        Ok(())
    }

    /// Close a position
    fn close_position(&mut self, symbol: &str, current_price: Decimal) -> Result<()> {
        let position = self.position_manager.get_position(symbol)
            .ok_or_else(|| anyhow::anyhow!("Position not found"))?;

        let position_size = position.entry_price * position.quantity;

        // Simulate fill
        let fill = self.simulate_fill(position.side.opposite(), current_price, position_size)?;

        // Close position
        let realized_pnl = self.position_manager.close_position(
            symbol,
            fill.price,
            fill.commission,
        )?;

        // Record trade
        self.risk_manager.record_trade(realized_pnl);
        self.equity += realized_pnl;

        // Store trade for analysis
        self.trades.push(BacktestTrade {
            entry_time: position.entry_time,
            exit_time: self.current_time,
            side: position.side,
            entry_price: position.entry_price,
            exit_price: fill.price,
            quantity: position.quantity,
            pnl: realized_pnl,
            fees: position.fees_paid + fill.commission,
        });

        Ok(())
    }

    /// Simulate order fill with slippage and commission
    fn simulate_fill(
        &self,
        side: Side,
        price: Decimal,
        notional: Decimal,
    ) -> Result<SimulatedFill> {
        // Add slippage (unfavorable for us)
        let slippage_factor = self.config.slippage_bps / Decimal::from(10000);
        let slippage = match side {
            Side::Buy => price * slippage_factor,   // Pay more
            Side::Sell => -(price * slippage_factor), // Receive less
        };

        let filled_price = price + slippage;
        let quantity = notional / filled_price;
        let commission = notional * (self.config.commission_bps / Decimal::from(10000));

        Ok(SimulatedFill {
            price: filled_price,
            quantity,
            slippage: slippage.abs(),
            commission,
        })
    }

    /// Record current equity
    fn record_equity(&mut self) {
        let current_price = self.orderbook.get_mid_price()
            .unwrap_or(Decimal::ZERO);

        let unrealized_pnl = self.position_manager.total_unrealized_pnl(
            &[(self.config.symbol.clone(), current_price)]
        );

        let total_equity = self.equity + unrealized_pnl;
        self.equity_curve.push((self.current_time, total_equity));
    }

    /// Get backtest results
    pub fn get_results(&self) -> BacktestResults {
        BacktestResults::new(
            self.config.clone(),
            self.trades.clone(),
            self.equity_curve.clone(),
            self.equity,
        )
    }
}

/// Individual trade record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestTrade {
    pub entry_time: SystemTime,
    pub exit_time: SystemTime,
    pub side: Side,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub pnl: Decimal,
    pub fees: Decimal,
}

/// Backtest results with metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResults {
    pub config: BacktestConfig,
    pub trades: Vec<BacktestTrade>,
    pub equity_curve: Vec<(SystemTime, Decimal)>,
    pub final_equity: Decimal,
    pub total_return: Decimal,
    pub total_return_pct: Decimal,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub average_win: Decimal,
    pub average_loss: Decimal,
    pub largest_win: Decimal,
    pub largest_loss: Decimal,
    pub max_drawdown: Decimal,
    pub max_drawdown_pct: Decimal,
    pub sharpe_ratio: f64,
}

impl BacktestResults {
    pub fn new(
        config: BacktestConfig,
        trades: Vec<BacktestTrade>,
        equity_curve: Vec<(SystemTime, Decimal)>,
        final_equity: Decimal,
    ) -> Self {
        let total_return = final_equity - config.initial_capital;
        let total_return_pct = (total_return / config.initial_capital) * Decimal::from(100);
        
        let winning_trades: Vec<_> = trades.iter()
            .filter(|t| t.pnl > Decimal::ZERO)
            .collect();
        let losing_trades: Vec<_> = trades.iter()
            .filter(|t| t.pnl < Decimal::ZERO)
            .collect();

        let win_rate = if trades.is_empty() {
            0.0
        } else {
            winning_trades.len() as f64 / trades.len() as f64
        };

        let total_wins: Decimal = winning_trades.iter().map(|t| t.pnl).sum();
        let total_losses: Decimal = losing_trades.iter().map(|t| t.pnl.abs()).sum();

        let profit_factor = if total_losses.is_zero() {
            if total_wins.is_zero() { 0.0 } else { f64::INFINITY }
        } else {
            (total_wins / total_losses).to_string().parse().unwrap_or(0.0)
        };

        let average_win = if winning_trades.is_empty() {
            Decimal::ZERO
        } else {
            total_wins / Decimal::from(winning_trades.len())
        };

        let average_loss = if losing_trades.is_empty() {
            Decimal::ZERO
        } else {
            total_losses / Decimal::from(losing_trades.len())
        };

        let largest_win = winning_trades.iter()
            .map(|t| t.pnl)
            .max()
            .unwrap_or(Decimal::ZERO);

        let largest_loss = losing_trades.iter()
            .map(|t| t.pnl)
            .min()
            .unwrap_or(Decimal::ZERO);

        // Calculate max drawdown
        let (max_dd, max_dd_pct) = Self::calculate_max_drawdown(&equity_curve, config.initial_capital);

        // Calculate Sharpe ratio (simplified, assuming 0 risk-free rate)
        let sharpe_ratio = Self::calculate_sharpe_ratio(&trades);

        Self {
            config,
            trades: trades.clone(),
            equity_curve,
            final_equity,
            total_return,
            total_return_pct,
            total_trades: trades.len(),
            winning_trades: winning_trades.len(),
            losing_trades: losing_trades.len(),
            win_rate,
            profit_factor,
            average_win,
            average_loss,
            largest_win,
            largest_loss,
            max_drawdown: max_dd,
            max_drawdown_pct: max_dd_pct,
            sharpe_ratio,
        }
    }

    fn calculate_max_drawdown(
        equity_curve: &[(SystemTime, Decimal)],
        initial_capital: Decimal,
    ) -> (Decimal, Decimal) {
        let mut peak = initial_capital;
        let mut max_dd = Decimal::ZERO;

        for (_, equity) in equity_curve {
            if *equity > peak {
                peak = *equity;
            }

            let drawdown = peak - *equity;
            if drawdown > max_dd {
                max_dd = drawdown;
            }
        }

        let max_dd_pct = if peak.is_zero() {
            Decimal::ZERO
        } else {
            (max_dd / peak) * Decimal::from(100)
        };

        (max_dd, max_dd_pct)
    }

    fn calculate_sharpe_ratio(trades: &[BacktestTrade]) -> f64 {
        if trades.len() < 2 {
            return 0.0;
        }

        let returns: Vec<f64> = trades.iter()
            .map(|t| t.pnl.to_string().parse::<f64>().unwrap_or(0.0))
            .collect();

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / returns.len() as f64;

        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            0.0
        } else {
            mean_return / std_dev
        }
    }

    /// Print results summary
    pub fn print_summary(&self) {
        println!("\n╔════════════════════════════════════════════════╗");
        println!("║         BACKTEST RESULTS SUMMARY               ║");
        println!("╠════════════════════════════════════════════════╣");
        println!("║ Symbol: {:<39} ║", self.config.symbol);
        println!("║ Initial Capital: ${:<28} ║", self.config.initial_capital);
        println!("║ Final Equity: ${:<31} ║", self.final_equity);
        println!("║ Total Return: ${:<30} ║", self.total_return);
        println!("║ Return %: {:<36.2}% ║", self.total_return_pct);
        println!("╠════════════════════════════════════════════════╣");
        println!("║ Total Trades: {:<34} ║", self.total_trades);
        println!("║ Winning Trades: {:<32} ║", self.winning_trades);
        println!("║ Losing Trades: {:<33} ║", self.losing_trades);
        println!("║ Win Rate: {:<36.2}% ║", self.win_rate * 100.0);
        println!("║ Profit Factor: {:<33.2} ║", self.profit_factor);
        println!("╠════════════════════════════════════════════════╣");
        println!("║ Average Win: ${:<32} ║", self.average_win);
        println!("║ Average Loss: ${:<31} ║", self.average_loss);
        println!("║ Largest Win: ${:<32} ║", self.largest_win);
        println!("║ Largest Loss: ${:<31} ║", self.largest_loss);
        println!("╠════════════════════════════════════════════════╣");
        println!("║ Max Drawdown: ${:<30} ║", self.max_drawdown);
        println!("║ Max Drawdown %: {:<29.2}% ║", self.max_drawdown_pct);
        println!("║ Sharpe Ratio: {:<34.2} ║", self.sharpe_ratio);
        println!("╚════════════════════════════════════════════════╝\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backtest_engine_creation() {
        let config = BacktestConfig::default();
        let engine = BacktestEngine::new(config);
        
        assert_eq!(engine.equity, Decimal::from(10000));
        assert_eq!(engine.position_manager.position_count(), 0);
    }
}
