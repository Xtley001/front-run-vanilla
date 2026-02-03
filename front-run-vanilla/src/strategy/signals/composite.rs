use crate::data::{Signal, Side};
use std::time::SystemTime;

/// Composite signal combining multiple signal sources
/// 
/// Aggregates signals from different detectors and determines
/// if we should execute a trade based on:
/// 1. Primary signal strength
/// 2. Number of confirming signals
/// 3. Overall confidence
#[derive(Debug, Clone)]
pub struct CompositeSignal {
    /// Strongest signal (drives the decision)
    pub primary: Signal,
    
    /// Confirming signals (same direction)
    pub confirming: Vec<Signal>,
    
    /// Overall signal strength
    pub overall_strength: f64,
    
    /// Trading direction
    pub direction: Side,
    
    /// Composite confidence (0.0 to 1.0)
    pub confidence: f64,
    
    /// Timestamp of signal generation
    pub timestamp: SystemTime,
}

impl CompositeSignal {
    /// Check if this signal is strong enough to trade
    pub fn is_tradeable(&self, min_confirming: usize) -> bool {
        self.confirming.len() >= min_confirming && self.confidence >= 0.5
    }
}

/// Signal aggregator
pub struct SignalAggregator {
    /// Minimum signal strength for primary (z-score)
    primary_threshold: f64,
    
    /// Minimum signal strength for confirming (z-score)
    confirming_threshold: f64,
    
    /// Minimum number of confirming signals required
    min_confirming: usize,
}

impl SignalAggregator {
    /// Create new signal aggregator
    /// 
    /// # Arguments
    /// * `primary_threshold` - Min strength for primary signal (e.g., 3.0)
    /// * `confirming_threshold` - Min strength for confirming (e.g., 1.5)
    /// * `min_confirming` - Min number of confirming signals (e.g., 2)
    pub fn new(
        primary_threshold: f64,
        confirming_threshold: f64,
        min_confirming: usize,
    ) -> Self {
        Self {
            primary_threshold,
            confirming_threshold,
            min_confirming,
        }
    }

    /// Aggregate multiple signals into a composite signal
    /// 
    /// Returns Some(CompositeSignal) if signals meet criteria, None otherwise
    pub fn aggregate(&self, signals: Vec<Signal>) -> Option<CompositeSignal> {
        if signals.is_empty() {
            return None;
        }

        // 1. Find primary signal (strongest absolute signal)
        let primary = signals.iter()
            .max_by(|a, b| {
                a.abs_strength()
                    .partial_cmp(&b.abs_strength())
                    .unwrap()
            })?
            .clone();

        // 2. Check if primary meets threshold
        if primary.abs_strength() < self.primary_threshold {
            return None;
        }

        // 3. Find confirming signals (same direction, above threshold)
        let confirming: Vec<Signal> = signals.iter()
            .filter(|s| {
                s.direction == primary.direction
                    && s.abs_strength() >= self.confirming_threshold
                    && s.timestamp != primary.timestamp  // Don't count self
            })
            .cloned()
            .collect();

        // 4. Check minimum confirming signals
        if confirming.len() < self.min_confirming {
            return None;
        }

        // 5. Calculate composite confidence
        let confidence = self.calculate_composite_confidence(&primary, &confirming);

        // 6. Calculate overall strength (weighted average)
        let overall_strength = self.calculate_overall_strength(&primary, &confirming);

        Some(CompositeSignal {
            direction: primary.direction,
            overall_strength,
            confidence,
            timestamp: SystemTime::now(),
            primary: primary.clone(),
            confirming,
        })
    }

    /// Calculate composite confidence
    /// 
    /// Factors:
    /// - Primary signal confidence (40% weight)
    /// - Number of confirming signals (30% weight)
    /// - Average confirming signal confidence (30% weight)
    fn calculate_composite_confidence(
        &self,
        primary: &Signal,
        confirming: &[Signal],
    ) -> f64 {
        // Primary signal confidence
        let primary_conf = primary.confidence * 0.4;

        // Confirming count factor (more confirming = higher confidence)
        let count_factor = (confirming.len() as f64 / (self.min_confirming as f64 + 2.0)).min(1.0);
        let count_conf = count_factor * 0.3;

        // Average confirming confidence
        let avg_confirming_conf = if confirming.is_empty() {
            0.0
        } else {
            confirming.iter().map(|s| s.confidence).sum::<f64>() / confirming.len() as f64
        };
        let confirming_conf = avg_confirming_conf * 0.3;

        (primary_conf + count_conf + confirming_conf).min(1.0)
    }

    /// Calculate overall signal strength (weighted average)
    fn calculate_overall_strength(
        &self,
        primary: &Signal,
        confirming: &[Signal],
    ) -> f64 {
        // Primary gets 60% weight
        let primary_weighted = primary.strength * 0.6;

        // Confirming get 40% weight (split evenly)
        let confirming_weighted = if confirming.is_empty() {
            0.0
        } else {
            let avg_strength = confirming.iter().map(|s| s.strength).sum::<f64>() 
                / confirming.len() as f64;
            avg_strength * 0.4
        };

        primary_weighted + confirming_weighted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::SignalComponent;

    fn create_signal(strength: f64, direction: Side, confidence: f64) -> Signal {
        Signal {
            strength,
            direction,
            confidence,
            timestamp: SystemTime::now(),
            components: vec![],
        }
    }

    #[test]
    fn test_strong_composite_signal() {
        let aggregator = SignalAggregator::new(3.0, 1.5, 2);

        let signals = vec![
            create_signal(4.0, Side::Buy, 0.8),   // Strong primary
            create_signal(2.0, Side::Buy, 0.6),   // Confirming 1
            create_signal(2.5, Side::Buy, 0.7),   // Confirming 2
        ];

        let composite = aggregator.aggregate(signals);
        
        assert!(composite.is_some());
        let sig = composite.unwrap();
        assert_eq!(sig.direction, Side::Buy);
        assert_eq!(sig.confirming.len(), 2);
        assert!(sig.confidence > 0.5);
        assert!(sig.is_tradeable(2));
    }

    #[test]
    fn test_insufficient_primary_strength() {
        let aggregator = SignalAggregator::new(3.0, 1.5, 2);

        let signals = vec![
            create_signal(2.0, Side::Buy, 0.8),   // Too weak
            create_signal(2.0, Side::Buy, 0.6),
            create_signal(2.5, Side::Buy, 0.7),
        ];

        let composite = aggregator.aggregate(signals);
        assert!(composite.is_none());
    }

    #[test]
    fn test_insufficient_confirming_signals() {
        let aggregator = SignalAggregator::new(3.0, 1.5, 2);

        let signals = vec![
            create_signal(4.0, Side::Buy, 0.8),   // Strong primary
            create_signal(2.0, Side::Buy, 0.6),   // Only 1 confirming
        ];

        let composite = aggregator.aggregate(signals);
        assert!(composite.is_none());  // Needs 2 confirming
    }

    #[test]
    fn test_conflicting_directions() {
        let aggregator = SignalAggregator::new(3.0, 1.5, 2);

        let signals = vec![
            create_signal(4.0, Side::Buy, 0.8),    // Primary BUY
            create_signal(-2.0, Side::Sell, 0.6),  // Conflicting SELL
            create_signal(-2.5, Side::Sell, 0.7),  // Conflicting SELL
        ];

        let composite = aggregator.aggregate(signals);
        
        // Should not have confirming signals due to direction mismatch
        assert!(composite.is_none());
    }

    #[test]
    fn test_mixed_but_sufficient_confirming() {
        let aggregator = SignalAggregator::new(3.0, 1.5, 2);

        let signals = vec![
            create_signal(4.0, Side::Buy, 0.8),    // Primary BUY
            create_signal(2.0, Side::Buy, 0.6),    // Confirming 1
            create_signal(2.5, Side::Buy, 0.7),    // Confirming 2
            create_signal(-1.0, Side::Sell, 0.3),  // Weak opposing (ignored)
        ];

        let composite = aggregator.aggregate(signals);
        
        assert!(composite.is_some());
        let sig = composite.unwrap();
        assert_eq!(sig.confirming.len(), 2);
        assert_eq!(sig.direction, Side::Buy);
    }

    #[test]
    fn test_high_confidence_calculation() {
        let aggregator = SignalAggregator::new(3.0, 1.5, 2);

        let signals = vec![
            create_signal(5.0, Side::Buy, 0.9),   // Very strong primary
            create_signal(3.0, Side::Buy, 0.8),   // Strong confirming 1
            create_signal(3.5, Side::Buy, 0.85),  // Strong confirming 2
            create_signal(2.0, Side::Buy, 0.7),   // Additional confirming
        ];

        let composite = aggregator.aggregate(signals).unwrap();
        
        // High confidence due to strong signals and multiple confirmations
        assert!(composite.confidence > 0.7);
    }

    #[test]
    fn test_empty_signals() {
        let aggregator = SignalAggregator::new(3.0, 1.5, 2);
        let composite = aggregator.aggregate(vec![]);
        assert!(composite.is_none());
    }
}
