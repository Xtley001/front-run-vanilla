use rust_decimal::Decimal;
use std::collections::VecDeque;
use std::time::{SystemTime, Duration};
use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};

/// Risk limit violation error
#[derive(Debug, Clone)]
pub struct RiskViolation {
    pub reason: String,
    pub severity: ViolationSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViolationSeverity {
    Warning,   // Log but allow trade
    Block,     // Prevent trade
    Emergency, // Close all positions
}

/// Risk limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    // Position limits
    pub max_position_size: Decimal,
    pub max_portfolio_exposure: Decimal,
    
    // Loss limits
    pub max_daily_loss: Decimal,
    pub max_drawdown_percent: Decimal,
    
    // Rate limits
    pub max_trades_per_hour: usize,
    pub max_trades_per_day: usize,
    
    // Latency limits
    pub max_acceptable_latency_ms: u64,
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_position_size: Decimal::from(5000),
            max_portfolio_exposure: Decimal::from(10000),
            max_daily_loss: Decimal::from(500),
            max_drawdown_percent: Decimal::from(10),
            max_trades_per_hour: 30,
            max_trades_per_day: 200,
            max_acceptable_latency_ms: 500,
        }
    }
}

/// Risk manager enforcing all limits
pub struct RiskManager {
    limits: RiskLimits,
    
    // Daily tracking
    daily_pnl: Decimal,
    daily_trades: usize,
    day_start: SystemTime,
    
    // Hourly tracking
    hourly_trades: VecDeque<SystemTime>,
    
    // Drawdown tracking
    peak_equity: Decimal,
    current_equity: Decimal,
    
    // Latency tracking
    recent_latencies: VecDeque<u64>,
    
    // Circuit breaker state
    trading_halted: bool,
    halt_reason: Option<String>,
}

impl RiskManager {
    pub fn new(limits: RiskLimits, initial_equity: Decimal) -> Self {
        Self {
            limits,
            daily_pnl: Decimal::ZERO,
            daily_trades: 0,
            day_start: SystemTime::now(),
            hourly_trades: VecDeque::new(),
            peak_equity: initial_equity,
            current_equity: initial_equity,
            recent_latencies: VecDeque::new(),
            trading_halted: false,
            halt_reason: None,
        }
    }

    /// Check if a new position can be opened
    pub fn can_open_position(
        &mut self,
        position_size: Decimal,
        current_exposure: Decimal,
    ) -> Result<(), RiskViolation> {
        // Check circuit breaker
        if self.trading_halted {
            return Err(RiskViolation {
                reason: format!("Trading halted: {}", 
                    self.halt_reason.as_ref().unwrap_or(&"Unknown".to_string())),
                severity: ViolationSeverity::Emergency,
            });
        }

        // Check position size limit
        if position_size > self.limits.max_position_size {
            return Err(RiskViolation {
                reason: format!(
                    "Position size {} exceeds limit {}",
                    position_size, self.limits.max_position_size
                ),
                severity: ViolationSeverity::Block,
            });
        }

        // Check portfolio exposure
        let new_exposure = current_exposure + position_size;
        if new_exposure > self.limits.max_portfolio_exposure {
            return Err(RiskViolation {
                reason: format!(
                    "Portfolio exposure {} exceeds limit {}",
                    new_exposure, self.limits.max_portfolio_exposure
                ),
                severity: ViolationSeverity::Block,
            });
        }

        // Check daily loss limit
        if self.daily_pnl < -self.limits.max_daily_loss {
            self.halt_trading("Daily loss limit exceeded");
            return Err(RiskViolation {
                reason: format!(
                    "Daily loss {} exceeds limit {}",
                    self.daily_pnl, self.limits.max_daily_loss
                ),
                severity: ViolationSeverity::Emergency,
            });
        }

        // Check drawdown
        let drawdown = self.calculate_drawdown();
        if drawdown > self.limits.max_drawdown_percent {
            self.halt_trading("Drawdown limit exceeded");
            return Err(RiskViolation {
                reason: format!(
                    "Drawdown {}% exceeds limit {}%",
                    drawdown, self.limits.max_drawdown_percent
                ),
                severity: ViolationSeverity::Emergency,
            });
        }

        // Check hourly trade limit
        self.cleanup_old_trades();
        if self.hourly_trades.len() >= self.limits.max_trades_per_hour {
            return Err(RiskViolation {
                reason: format!(
                    "Hourly trade limit {} reached",
                    self.limits.max_trades_per_hour
                ),
                severity: ViolationSeverity::Block,
            });
        }

        // Check daily trade limit
        if self.daily_trades >= self.limits.max_trades_per_day {
            return Err(RiskViolation {
                reason: format!(
                    "Daily trade limit {} reached",
                    self.limits.max_trades_per_day
                ),
                severity: ViolationSeverity::Block,
            });
        }

        // Check latency
        if let Some(avg_latency) = self.average_latency() {
            if avg_latency > self.limits.max_acceptable_latency_ms {
                return Err(RiskViolation {
                    reason: format!(
                        "Average latency {}ms exceeds limit {}ms",
                        avg_latency, self.limits.max_acceptable_latency_ms
                    ),
                    severity: ViolationSeverity::Warning,
                });
            }
        }

        Ok(())
    }

    /// Record a trade
    pub fn record_trade(&mut self, pnl: Decimal) {
        self.hourly_trades.push_back(SystemTime::now());
        self.daily_trades += 1;
        self.daily_pnl += pnl;
        self.current_equity += pnl;

        // Update peak equity
        if self.current_equity > self.peak_equity {
            self.peak_equity = self.current_equity;
        }

        // Reset daily counters if new day
        self.check_new_day();
    }

    /// Record execution latency
    pub fn record_latency(&mut self, latency_ms: u64) {
        self.recent_latencies.push_back(latency_ms);
        if self.recent_latencies.len() > 100 {
            self.recent_latencies.pop_front();
        }

        // Check for consistent high latency
        if self.recent_latencies.len() >= 10 {
            let recent_high_latency = self.recent_latencies.iter()
                .rev()
                .take(10)
                .filter(|&&l| l > self.limits.max_acceptable_latency_ms)
                .count();

            if recent_high_latency >= 8 {
                self.halt_trading("Consistent high latency detected");
            }
        }
    }

    /// Calculate current drawdown percentage
    fn calculate_drawdown(&self) -> Decimal {
        if self.peak_equity.is_zero() {
            return Decimal::ZERO;
        }

        let drawdown = (self.peak_equity - self.current_equity) / self.peak_equity;
        drawdown * Decimal::from(100)
    }

    /// Get average latency
    fn average_latency(&self) -> Option<u64> {
        if self.recent_latencies.is_empty() {
            return None;
        }

        let sum: u64 = self.recent_latencies.iter().sum();
        Some(sum / self.recent_latencies.len() as u64)
    }

    /// Remove trades older than 1 hour
    fn cleanup_old_trades(&mut self) {
        let one_hour_ago = SystemTime::now() - Duration::from_secs(3600);
        
        while let Some(&trade_time) = self.hourly_trades.front() {
            if trade_time < one_hour_ago {
                self.hourly_trades.pop_front();
            } else {
                break;
            }
        }
    }

    /// Check if new day and reset counters
    fn check_new_day(&mut self) {
        let elapsed = SystemTime::now()
            .duration_since(self.day_start)
            .unwrap_or(Duration::ZERO);

        if elapsed.as_secs() >= 86400 {  // 24 hours
            self.daily_pnl = Decimal::ZERO;
            self.daily_trades = 0;
            self.day_start = SystemTime::now();
        }
    }

    /// Halt all trading
    pub fn halt_trading(&mut self, reason: &str) {
        self.trading_halted = true;
        self.halt_reason = Some(reason.to_string());
    }

    /// Resume trading (manual override)
    pub fn resume_trading(&mut self) {
        self.trading_halted = false;
        self.halt_reason = None;
    }

    /// Check if trading is halted
    pub fn is_halted(&self) -> bool {
        self.trading_halted
    }

    /// Get halt reason
    pub fn halt_reason(&self) -> Option<&str> {
        self.halt_reason.as_deref()
    }

    /// Get current risk metrics
    pub fn get_metrics(&self) -> RiskMetrics {
        RiskMetrics {
            daily_pnl: self.daily_pnl,
            daily_trades: self.daily_trades,
            hourly_trades: self.hourly_trades.len(),
            drawdown_percent: self.calculate_drawdown(),
            current_equity: self.current_equity,
            peak_equity: self.peak_equity,
            average_latency_ms: self.average_latency(),
            trading_halted: self.trading_halted,
        }
    }
}

/// Risk metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub daily_pnl: Decimal,
    pub daily_trades: usize,
    pub hourly_trades: usize,
    pub drawdown_percent: Decimal,
    pub current_equity: Decimal,
    pub peak_equity: Decimal,
    pub average_latency_ms: Option<u64>,
    pub trading_halted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_position_size_limit() {
        let limits = RiskLimits::default();
        let mut manager = RiskManager::new(limits, dec!(10000));

        // Within limit
        assert!(manager.can_open_position(dec!(4000), dec!(0)).is_ok());

        // Exceeds limit
        let result = manager.can_open_position(dec!(6000), dec!(0));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().severity, ViolationSeverity::Block);
    }

    #[test]
    fn test_portfolio_exposure_limit() {
        let limits = RiskLimits::default();
        let mut manager = RiskManager::new(limits, dec!(10000));

        // Current exposure + new position exceeds limit
        let result = manager.can_open_position(dec!(3000), dec!(8000));
        assert!(result.is_err());
    }

    #[test]
    fn test_daily_loss_limit() {
        let limits = RiskLimits::default();
        let mut manager = RiskManager::new(limits, dec!(10000));

        // Record losing trades
        manager.record_trade(dec!(-200));
        manager.record_trade(dec!(-200));
        manager.record_trade(dec!(-150));

        // Should halt trading
        let result = manager.can_open_position(dec!(1000), dec!(0));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().severity, ViolationSeverity::Emergency);
        assert!(manager.is_halted());
    }

    #[test]
    fn test_drawdown_limit() {
        let limits = RiskLimits::default();
        let mut manager = RiskManager::new(limits, dec!(10000));

        // Simulate 11% drawdown
        manager.record_trade(dec!(-1100));

        let result = manager.can_open_position(dec!(1000), dec!(0));
        assert!(result.is_err());
        assert!(manager.is_halted());
    }

    #[test]
    fn test_hourly_trade_limit() {
        let mut limits = RiskLimits::default();
        limits.max_trades_per_hour = 5;
        let mut manager = RiskManager::new(limits, dec!(10000));

        // Record 5 trades
        for _ in 0..5 {
            manager.record_trade(dec!(10));
        }

        // 6th trade should be blocked
        let result = manager.can_open_position(dec!(1000), dec!(0));
        assert!(result.is_err());
    }

    #[test]
    fn test_latency_tracking() {
        let limits = RiskLimits::default();
        let mut manager = RiskManager::new(limits, dec!(10000));

        // Record normal latencies
        for _ in 0..5 {
            manager.record_latency(50);
        }

        assert!(manager.average_latency().unwrap() == 50);

        // Record high latencies
        for _ in 0..10 {
            manager.record_latency(600);  // Above limit
        }

        // Should halt trading after consistent high latency
        assert!(manager.is_halted());
    }

    #[test]
    fn test_resume_trading() {
        let limits = RiskLimits::default();
        let mut manager = RiskManager::new(limits, dec!(10000));

        manager.halt_trading("Test halt");
        assert!(manager.is_halted());

        manager.resume_trading();
        assert!(!manager.is_halted());
    }
}
