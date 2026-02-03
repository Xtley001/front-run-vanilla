use crate::data::{OrderBook, Signal, SignalComponent, Side};
use std::collections::VecDeque;
use std::time::SystemTime;

/// Order book imbalance detector
/// 
/// PRIMARY SIGNAL: Detects when bid/ask depth ratio deviates significantly
/// from its rolling average, indicating potential whale activity.
/// 
/// Algorithm:
/// 1. Calculate bid_depth / ask_depth ratio for top N levels
/// 2. Maintain rolling window of ratios
/// 3. Calculate z-score (standard deviations from mean)
/// 4. If z-score > threshold, generate signal
pub struct ImbalanceDetector {
    /// Number of price levels to analyze
    levels: usize,
    
    /// Rolling window of imbalance ratios
    history: VecDeque<f64>,
    
    /// Window size for rolling statistics
    window_size: usize,
    
    /// Z-score threshold for signal generation (e.g., 3.0 = 3 sigma)
    threshold: f64,
    
    /// Minimum samples needed before generating signals
    min_samples: usize,
}

impl ImbalanceDetector {
    /// Create new imbalance detector
    /// 
    /// # Arguments
    /// * `levels` - Number of order book levels to analyze (typically 5-10)
    /// * `window_size` - Rolling window size for statistics (typically 100-200)
    /// * `threshold` - Z-score threshold for signal (typically 2.5-3.5)
    pub fn new(levels: usize, window_size: usize, threshold: f64) -> Self {
        Self {
            levels,
            history: VecDeque::with_capacity(window_size),
            window_size,
            threshold,
            min_samples: window_size / 2,  // Need at least 50% of window
        }
    }

    /// Calculate imbalance signal from current order book state
    /// 
    /// Returns Some(Signal) if imbalance exceeds threshold, None otherwise
    pub fn calculate_signal(&mut self, orderbook: &OrderBook) -> Option<Signal> {
        // 1. Calculate current imbalance ratio
        let ratio = orderbook.calculate_imbalance(self.levels)?;

        // 2. Add to history
        self.history.push_back(ratio);
        if self.history.len() > self.window_size {
            self.history.pop_front();
        }

        // 3. Need minimum samples before generating signals
        if self.history.len() < self.min_samples {
            return None;
        }

        // 4. Calculate rolling statistics
        let mean = self.calculate_mean();
        let stddev = self.calculate_stddev(mean);

        // Avoid division by zero
        if stddev < 1e-6 {
            return None;
        }

        // 5. Calculate z-score (how many standard deviations from mean)
        let z_score = (ratio - mean) / stddev;

        // 6. Check if signal exceeds threshold
        if z_score.abs() < self.threshold {
            return None;
        }

        // 7. Determine direction
        // Positive z-score = more bids than usual = bullish = BUY
        // Negative z-score = more asks than usual = bearish = SELL
        let direction = if z_score > 0.0 {
            Side::Buy
        } else {
            Side::Sell
        };

        // 8. Calculate confidence (0.0 to 1.0)
        // Higher deviation from threshold = higher confidence
        let confidence = (z_score.abs() / (self.threshold + 1.0)).min(1.0);

        // 9. Create signal components for analysis
        let components = vec![
            SignalComponent::new("imbalance_ratio", ratio, 1.0),
            SignalComponent::new("mean", mean, 0.0),
            SignalComponent::new("stddev", stddev, 0.0),
            SignalComponent::new("z_score", z_score, 1.0),
        ];

        Some(Signal {
            strength: z_score,
            direction,
            confidence,
            timestamp: SystemTime::now(),
            components,
        })
    }

    /// Calculate mean of history
    fn calculate_mean(&self) -> f64 {
        if self.history.is_empty() {
            return 0.0;
        }
        self.history.iter().sum::<f64>() / self.history.len() as f64
    }

    /// Calculate standard deviation of history
    fn calculate_stddev(&self, mean: f64) -> f64 {
        if self.history.len() < 2 {
            return 0.0;
        }

        let variance = self.history.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / self.history.len() as f64;

        variance.sqrt()
    }

    /// Get current statistics for debugging
    pub fn get_stats(&self) -> ImbalanceStats {
        let mean = self.calculate_mean();
        let stddev = self.calculate_stddev(mean);
        
        ImbalanceStats {
            current_ratio: self.history.back().copied(),
            mean,
            stddev,
            sample_count: self.history.len(),
        }
    }

    /// Reset the detector (clears history)
    pub fn reset(&mut self) {
        self.history.clear();
    }
}

/// Imbalance statistics for monitoring
#[derive(Debug, Clone)]
pub struct ImbalanceStats {
    pub current_ratio: Option<f64>,
    pub mean: f64,
    pub stddev: f64,
    pub sample_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OrderBook;
    use rust_decimal_macros::dec;

    #[test]
    fn test_bullish_imbalance_signal() {
        let mut detector = ImbalanceDetector::new(5, 100, 3.0);
        let ob = OrderBook::new("BTCUSDT");

        // Build normal baseline first
        for _ in 0..50 {
            ob.update_level(Side::Buy, dec!(100.0), dec!(5.0)).unwrap();
            ob.update_level(Side::Sell, dec!(101.0), dec!(5.0)).unwrap();
            detector.calculate_signal(&ob);
        }

        // Now create heavy bid imbalance
        ob.update_level(Side::Buy, dec!(100.0), dec!(50.0)).unwrap();  // 10x bid
        ob.update_level(Side::Sell, dec!(101.0), dec!(2.0)).unwrap();   // Low ask

        let signal = detector.calculate_signal(&ob);
        
        assert!(signal.is_some());
        let sig = signal.unwrap();
        assert_eq!(sig.direction, Side::Buy);
        assert!(sig.strength > 0.0);
        assert!(sig.confidence > 0.5);
    }

    #[test]
    fn test_bearish_imbalance_signal() {
        let mut detector = ImbalanceDetector::new(5, 100, 3.0);
        let ob = OrderBook::new("BTCUSDT");

        // Build baseline
        for _ in 0..50 {
            ob.update_level(Side::Buy, dec!(100.0), dec!(5.0)).unwrap();
            ob.update_level(Side::Sell, dec!(101.0), dec!(5.0)).unwrap();
            detector.calculate_signal(&ob);
        }

        // Heavy ask imbalance
        ob.update_level(Side::Buy, dec!(100.0), dec!(2.0)).unwrap();
        ob.update_level(Side::Sell, dec!(101.0), dec!(50.0)).unwrap();

        let signal = detector.calculate_signal(&ob);
        
        assert!(signal.is_some());
        let sig = signal.unwrap();
        assert_eq!(sig.direction, Side::Sell);
        assert!(sig.strength < 0.0);
    }

    #[test]
    fn test_no_signal_balanced_book() {
        let mut detector = ImbalanceDetector::new(5, 100, 3.0);
        let ob = OrderBook::new("BTCUSDT");

        // Balanced book
        for _ in 0..100 {
            ob.update_level(Side::Buy, dec!(100.0), dec!(5.0)).unwrap();
            ob.update_level(Side::Sell, dec!(101.0), dec!(5.0)).unwrap();
            
            let signal = detector.calculate_signal(&ob);
            // Should not generate signal for balanced book
            if signal.is_some() {
                assert!(signal.unwrap().strength.abs() < 3.0);
            }
        }
    }

    #[test]
    fn test_minimum_samples_required() {
        let mut detector = ImbalanceDetector::new(5, 100, 3.0);
        let ob = OrderBook::new("BTCUSDT");

        ob.update_level(Side::Buy, dec!(100.0), dec!(50.0)).unwrap();
        ob.update_level(Side::Sell, dec!(101.0), dec!(1.0)).unwrap();

        // Should not generate signal with insufficient samples
        let signal = detector.calculate_signal(&ob);
        assert!(signal.is_none());
    }

    #[test]
    fn test_statistics() {
        let mut detector = ImbalanceDetector::new(5, 100, 3.0);
        let ob = OrderBook::new("BTCUSDT");

        for _ in 0..60 {
            ob.update_level(Side::Buy, dec!(100.0), dec!(5.0)).unwrap();
            ob.update_level(Side::Sell, dec!(101.0), dec!(5.0)).unwrap();
            detector.calculate_signal(&ob);
        }

        let stats = detector.get_stats();
        assert_eq!(stats.sample_count, 60);
        assert!(stats.current_ratio.is_some());
        // Mean should be around 1.0 for balanced book
        assert!((stats.mean - 1.0).abs() < 0.1);
    }
}
