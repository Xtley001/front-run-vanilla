use crate::data::{Trade, Signal, SignalComponent, Side};
use rust_decimal::Decimal;
use std::collections::VecDeque;
use std::time::{SystemTime, Duration};

/// Aggressive flow analyzer
/// 
/// SECONDARY SIGNAL: Detects when aggressive buying or selling is occurring
/// by analyzing the trade stream for clusters of same-direction trades.
/// 
/// Algorithm:
/// 1. Maintain sliding window of recent trades
/// 2. Separate into aggressive buys vs sells
/// 3. Calculate flow imbalance: (buy_vol - sell_vol) / total_vol
/// 4. Apply time decay to give more weight to recent trades
/// 5. Generate signal if imbalance exceeds threshold
pub struct FlowAnalyzer {
    /// Recent trades window
    trades: VecDeque<Trade>,
    
    /// Maximum number of trades to analyze
    window_size: usize,
    
    /// Time window in milliseconds
    time_window_ms: u64,
    
    /// Flow imbalance threshold (e.g., 0.6 = 60% one-sided)
    threshold: f64,
    
    /// Decay factor for time weighting (e.g., 0.95 = 5% decay per trade)
    decay_factor: f64,
}

impl FlowAnalyzer {
    /// Create new flow analyzer
    /// 
    /// # Arguments
    /// * `window_size` - Number of trades to analyze (typically 20-50)
    /// * `time_window_ms` - Time window in milliseconds (typically 1000-5000)
    /// * `threshold` - Flow imbalance threshold (typically 0.5-0.7)
    pub fn new(window_size: usize, time_window_ms: u64, threshold: f64) -> Self {
        Self {
            trades: VecDeque::with_capacity(window_size),
            window_size,
            time_window_ms,
            threshold,
            decay_factor: 0.95,  // Recent trades have more weight
        }
    }

    /// Process new trade and calculate flow signal
    pub fn process_trade(&mut self, trade: Trade) -> Option<Signal> {
        // 1. Add trade to window
        self.trades.push_back(trade.clone());

        // 2. Remove old trades (both by count and time)
        self.cleanup_old_trades();

        // 3. Need minimum trades before generating signals
        if self.trades.len() < self.window_size / 4 {
            return None;
        }

        // 4. Calculate weighted flow imbalance
        let (buy_volume, sell_volume) = self.calculate_weighted_volumes();
        let total_volume = buy_volume + sell_volume;

        if total_volume.is_zero() {
            return None;
        }

        // 5. Calculate flow imbalance (-1.0 to 1.0)
        let imbalance = (buy_volume - sell_volume) / total_volume;
        let imbalance_f64 = imbalance.to_string().parse::<f64>().ok()?;

        // 6. Check threshold
        if imbalance_f64.abs() < self.threshold {
            return None;
        }

        // 7. Determine direction
        let direction = if imbalance_f64 > 0.0 {
            Side::Buy  // Aggressive buying
        } else {
            Side::Sell  // Aggressive selling
        };

        // 8. Calculate signal strength (z-score equivalent)
        // Normalize by threshold so threshold=1.0 gives strength=1.0
        let strength = imbalance_f64 / self.threshold;

        // 9. Confidence based on trade count and consistency
        let confidence = self.calculate_confidence(imbalance_f64);

        // 10. Build signal components
        let components = vec![
            SignalComponent::new("buy_volume", buy_volume.to_string().parse().unwrap(), 1.0),
            SignalComponent::new("sell_volume", sell_volume.to_string().parse().unwrap(), 1.0),
            SignalComponent::new("imbalance", imbalance_f64, 1.0),
            SignalComponent::new("trade_count", self.trades.len() as f64, 0.0),
        ];

        Some(Signal {
            strength,
            direction,
            confidence,
            timestamp: SystemTime::now(),
            components,
        })
    }

    /// Calculate weighted buy and sell volumes
    /// More recent trades have higher weight
    fn calculate_weighted_volumes(&self) -> (Decimal, Decimal) {
        let mut buy_volume = Decimal::ZERO;
        let mut sell_volume = Decimal::ZERO;
        let mut weight = 1.0;

        // Iterate from newest to oldest
        for trade in self.trades.iter().rev() {
            let weighted_qty = trade.quantity * Decimal::from_f64_retain(weight).unwrap();

            if trade.is_aggressive_buy() {
                buy_volume += weighted_qty;
            } else if trade.is_aggressive_sell() {
                sell_volume += weighted_qty;
            }

            // Apply decay for older trades
            weight *= self.decay_factor;
        }

        (buy_volume, sell_volume)
    }

    /// Calculate confidence based on trade consistency
    fn calculate_confidence(&self, imbalance: f64) -> f64 {
        // Higher trade count = higher confidence
        let count_factor = (self.trades.len() as f64 / self.window_size as f64).min(1.0);

        // Higher imbalance = higher confidence
        let imbalance_factor = (imbalance.abs() / 1.0).min(1.0);

        // Combine factors
        (count_factor * 0.3 + imbalance_factor * 0.7).min(1.0)
    }

    /// Remove trades that are too old (by time or count)
    fn cleanup_old_trades(&mut self) {
        let cutoff_time = SystemTime::now() - Duration::from_millis(self.time_window_ms);

        // Remove by time
        while let Some(trade) = self.trades.front() {
            if trade.timestamp < cutoff_time {
                self.trades.pop_front();
            } else {
                break;
            }
        }

        // Remove by count
        while self.trades.len() > self.window_size {
            self.trades.pop_front();
        }
    }

    /// Get current flow statistics
    pub fn get_stats(&self) -> FlowStats {
        let (buy_vol, sell_vol) = self.calculate_weighted_volumes();
        let total = buy_vol + sell_vol;

        let imbalance = if !total.is_zero() {
            ((buy_vol - sell_vol) / total).to_string().parse().ok()
        } else {
            None
        };

        FlowStats {
            trade_count: self.trades.len(),
            buy_volume: buy_vol,
            sell_volume: sell_vol,
            imbalance,
        }
    }

    /// Reset the analyzer
    pub fn reset(&mut self) {
        self.trades.clear();
    }
}

/// Flow statistics for monitoring
#[derive(Debug, Clone)]
pub struct FlowStats {
    pub trade_count: usize,
    pub buy_volume: Decimal,
    pub sell_volume: Decimal,
    pub imbalance: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_buy_trade(qty: Decimal) -> Trade {
        Trade {
            id: 1,
            price: dec!(100.0),
            quantity: qty,
            side: Side::Buy,
            timestamp: SystemTime::now(),
            is_buyer_maker: false,  // Aggressive buy
        }
    }

    fn create_sell_trade(qty: Decimal) -> Trade {
        Trade {
            id: 2,
            price: dec!(100.0),
            quantity: qty,
            side: Side::Sell,
            timestamp: SystemTime::now(),
            is_buyer_maker: true,  // Aggressive sell
        }
    }

    #[test]
    fn test_aggressive_buying_signal() {
        let mut analyzer = FlowAnalyzer::new(20, 5000, 0.6);

        // Create strong buying pressure
        for _ in 0..15 {
            analyzer.process_trade(create_buy_trade(dec!(1.0)));
        }
        for _ in 0..5 {
            analyzer.process_trade(create_sell_trade(dec!(1.0)));
        }

        let signal = analyzer.process_trade(create_buy_trade(dec!(1.0)));

        assert!(signal.is_some());
        let sig = signal.unwrap();
        assert_eq!(sig.direction, Side::Buy);
        assert!(sig.strength > 0.0);
    }

    #[test]
    fn test_aggressive_selling_signal() {
        let mut analyzer = FlowAnalyzer::new(20, 5000, 0.6);

        // Strong selling pressure
        for _ in 0..15 {
            analyzer.process_trade(create_sell_trade(dec!(1.0)));
        }
        for _ in 0..5 {
            analyzer.process_trade(create_buy_trade(dec!(1.0)));
        }

        let signal = analyzer.process_trade(create_sell_trade(dec!(1.0)));

        assert!(signal.is_some());
        let sig = signal.unwrap();
        assert_eq!(sig.direction, Side::Sell);
        assert!(sig.strength < 0.0);
    }

    #[test]
    fn test_balanced_flow_no_signal() {
        let mut analyzer = FlowAnalyzer::new(20, 5000, 0.6);

        // Balanced flow
        for _ in 0..10 {
            analyzer.process_trade(create_buy_trade(dec!(1.0)));
            analyzer.process_trade(create_sell_trade(dec!(1.0)));
        }

        let signal = analyzer.process_trade(create_buy_trade(dec!(1.0)));
        
        // Should not generate signal for balanced flow
        assert!(signal.is_none());
    }

    #[test]
    fn test_recent_trades_weighted_more() {
        let mut analyzer = FlowAnalyzer::new(20, 5000, 0.6);

        // Old sells
        for _ in 0..10 {
            analyzer.process_trade(create_sell_trade(dec!(1.0)));
        }

        // Recent large buys
        for _ in 0..5 {
            analyzer.process_trade(create_buy_trade(dec!(3.0)));
        }

        let stats = analyzer.get_stats();
        
        // Recent buys should be weighted more
        assert!(stats.buy_volume > stats.sell_volume);
    }

    #[test]
    fn test_minimum_trades_required() {
        let mut analyzer = FlowAnalyzer::new(20, 5000, 0.6);

        // Only a few trades
        analyzer.process_trade(create_buy_trade(dec!(10.0)));
        let signal = analyzer.process_trade(create_buy_trade(dec!(10.0)));

        // Should not signal with too few trades
        assert!(signal.is_none());
    }

    #[test]
    fn test_statistics() {
        let mut analyzer = FlowAnalyzer::new(20, 5000, 0.6);

        for _ in 0..10 {
            analyzer.process_trade(create_buy_trade(dec!(2.0)));
        }
        for _ in 0..5 {
            analyzer.process_trade(create_sell_trade(dec!(1.0)));
        }

        let stats = analyzer.get_stats();
        assert_eq!(stats.trade_count, 15);
        assert!(stats.imbalance.is_some());
        assert!(stats.imbalance.unwrap() > 0.0);  // More buys
    }
}
