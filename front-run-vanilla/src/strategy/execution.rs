use crate::data::{Side, Order};
use crate::exchange::BinanceRestClient;
use crate::risk::{Position, PositionManager, RiskManager};
use crate::strategy::CompositeSignal;
use rust_decimal::Decimal;
use std::time::{SystemTime, Instant};
use anyhow::{Result, anyhow};
use tracing::{info, warn, error};

/// Trade execution result
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub order_id: String,
    pub symbol: String,
    pub side: Side,
    pub executed_price: Decimal,
    pub executed_qty: Decimal,
    pub latency_ms: u64,
    pub fees: Decimal,
    pub timestamp: SystemTime,
}

/// Execution engine with latency tracking
pub struct ExecutionEngine {
    client: BinanceRestClient,
    position_manager: PositionManager,
    risk_manager: RiskManager,
    
    // Trading configuration
    symbol: String,
    base_position_size: Decimal,
    min_size_multiplier: Decimal,
    max_size_multiplier: Decimal,
    
    // Exit parameters
    take_profit_bps: Decimal,
    stop_loss_bps: Decimal,
    max_hold_time_ms: u64,
    
    // Fee rate (Binance Futures taker fee: 0.04%)
    taker_fee_rate: Decimal,
}

impl ExecutionEngine {
    pub fn new(
        client: BinanceRestClient,
        risk_manager: RiskManager,
        symbol: String,
        base_position_size: Decimal,
        take_profit_bps: Decimal,
        stop_loss_bps: Decimal,
        max_hold_time_ms: u64,
    ) -> Self {
        Self {
            client,
            position_manager: PositionManager::new(),
            risk_manager,
            symbol,
            base_position_size,
            min_size_multiplier: Decimal::from_f64_retain(0.5).unwrap(),
            max_size_multiplier: Decimal::from_f64_retain(2.0).unwrap(),
            take_profit_bps,
            stop_loss_bps,
            max_hold_time_ms,
            taker_fee_rate: Decimal::from_f64_retain(0.0004).unwrap(), // 0.04%
        }
    }

    /// Execute a trade based on composite signal
    pub async fn execute_signal(
        &mut self,
        signal: CompositeSignal,
        current_price: Decimal,
    ) -> Result<ExecutionResult> {
        let signal_time = Instant::now();

        // 1. Calculate position size based on confidence
        let position_size = self.calculate_position_size(signal.confidence);

        // 2. Check risk limits
        let current_exposure = self.position_manager.total_exposure();
        self.risk_manager.can_open_position(position_size, current_exposure)
            .map_err(|e| anyhow!("Risk check failed: {}", e.reason))?;

        // 3. Calculate quantity
        let quantity = position_size / current_price;

        info!(
            "Executing signal: {:?} | Size: {} | Qty: {} | Price: {}",
            signal.direction, position_size, quantity, current_price
        );

        // 4. Place market order
        let order_response = self.client
            .place_market_order(&self.symbol, signal.direction, quantity)
            .await?;

        let execution_latency = signal_time.elapsed().as_millis() as u64;

        // 5. Record latency
        self.risk_manager.record_latency(execution_latency);

        // 6. Parse execution result
        let executed_price = order_response.price.parse::<Decimal>()
            .map_err(|e| anyhow!("Failed to parse price: {}", e))?;
        let executed_qty = order_response.executed_qty.parse::<Decimal>()
            .map_err(|e| anyhow!("Failed to parse quantity: {}", e))?;

        // 7. Calculate fees
        let fees = executed_price * executed_qty * self.taker_fee_rate;

        // 8. Create position
        let position = Position::new(
            self.symbol.clone(),
            signal.direction,
            executed_price,
            executed_qty,
            fees,
        );

        self.position_manager.open_position(position)?;

        info!(
            "✅ Order executed | ID: {} | Price: {} | Qty: {} | Latency: {}ms",
            order_response.order_id, executed_price, executed_qty, execution_latency
        );

        Ok(ExecutionResult {
            order_id: order_response.order_id.to_string(),
            symbol: self.symbol.clone(),
            side: signal.direction,
            executed_price,
            executed_qty,
            latency_ms: execution_latency,
            fees,
            timestamp: SystemTime::now(),
        })
    }

    /// Check exit conditions for all open positions
    pub async fn check_exits(&mut self, current_price: Decimal) -> Result<()> {
        let positions = self.position_manager.open_positions().to_vec();

        for position in positions {
            if self.should_exit(&position, current_price) {
                self.close_position(&position.symbol, current_price).await?;
            }
        }

        Ok(())
    }

    /// Check if position should be exited
    fn should_exit(&self, position: &Position, current_price: Decimal) -> bool {
        // Take profit hit
        if position.take_profit_hit(current_price, self.take_profit_bps) {
            info!("Take profit hit for {}", position.symbol);
            return true;
        }

        // Stop loss hit
        if position.stop_loss_hit(current_price, self.stop_loss_bps) {
            info!("Stop loss hit for {}", position.symbol);
            return true;
        }

        // Time-based exit
        if position.is_expired(self.max_hold_time_ms) {
            info!("Position expired for {}", position.symbol);
            return true;
        }

        false
    }

    /// Close a position
    async fn close_position(&mut self, symbol: &str, current_price: Decimal) -> Result<Decimal> {
        let position = self.position_manager.get_position(symbol)
            .ok_or_else(|| anyhow!("Position not found: {}", symbol))?;

        info!(
            "Closing position: {} | Entry: {} | Current: {} | Qty: {}",
            symbol, position.entry_price, current_price, position.quantity
        );

        // Determine close side (opposite of entry)
        let close_side = position.side.opposite();

        // Place market order to close
        let order_response = self.client
            .place_market_order(symbol, close_side, position.quantity)
            .await?;

        // Parse execution price
        let exit_price = order_response.price.parse::<Decimal>()?;
        let exit_qty = order_response.executed_qty.parse::<Decimal>()?;

        // Calculate exit fees
        let exit_fees = exit_price * exit_qty * self.taker_fee_rate;

        // Close position and get realized PnL
        let realized_pnl = self.position_manager.close_position(symbol, exit_price, exit_fees)?;

        // Record trade for risk management
        self.risk_manager.record_trade(realized_pnl);

        info!(
            "✅ Position closed | Exit: {} | PnL: {} | Fees: {}",
            exit_price, realized_pnl, exit_fees
        );

        Ok(realized_pnl)
    }

    /// Calculate position size based on signal confidence
    fn calculate_position_size(&self, confidence: f64) -> Decimal {
        // Scale position size: 0.5x to 2.0x based on confidence (0.0 to 1.0)
        let confidence_decimal = Decimal::from_f64_retain(confidence).unwrap();
        
        // Linear scaling: 0.5 at confidence=0, 2.0 at confidence=1
        let multiplier = self.min_size_multiplier 
            + (self.max_size_multiplier - self.min_size_multiplier) * confidence_decimal;

        self.base_position_size * multiplier
    }

    /// Get position manager
    pub fn position_manager(&self) -> &PositionManager {
        &self.position_manager
    }

    /// Get risk manager
    pub fn risk_manager(&self) -> &RiskManager {
        &self.risk_manager
    }

    /// Get mutable risk manager
    pub fn risk_manager_mut(&mut self) -> &mut RiskManager {
        &mut self.risk_manager
    }

    /// Emergency close all positions
    pub async fn emergency_close_all(&mut self, current_price: Decimal) -> Result<()> {
        warn!("🚨 EMERGENCY: Closing all positions");

        let positions = self.position_manager.open_positions().to_vec();

        for position in positions {
            match self.close_position(&position.symbol, current_price).await {
                Ok(pnl) => {
                    info!("Emergency closed {} with PnL: {}", position.symbol, pnl);
                }
                Err(e) => {
                    error!("Failed to emergency close {}: {}", position.symbol, e);
                }
            }
        }

        Ok(())
    }

    /// Get trading statistics
    pub fn get_stats(&self) -> TradingStats {
        TradingStats {
            open_positions: self.position_manager.position_count(),
            closed_trades: self.position_manager.closed_positions().len(),
            total_realized_pnl: self.position_manager.total_realized_pnl(),
            total_fees: self.position_manager.total_fees(),
            win_rate: self.position_manager.win_rate(),
            average_trade_pnl: self.position_manager.average_trade_pnl(),
            risk_metrics: self.risk_manager.get_metrics(),
        }
    }
}

/// Trading statistics
#[derive(Debug, Clone)]
pub struct TradingStats {
    pub open_positions: usize,
    pub closed_trades: usize,
    pub total_realized_pnl: Decimal,
    pub total_fees: Decimal,
    pub win_rate: f64,
    pub average_trade_pnl: Decimal,
    pub risk_metrics: crate::risk::RiskMetrics,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_size_calculation() {
        let client = BinanceRestClient::new(
            "test".into(),
            "test".into(),
            "https://test".into(),
        );
        let risk_manager = RiskManager::new(
            crate::risk::RiskLimits::default(),
            Decimal::from(10000),
        );

        let engine = ExecutionEngine::new(
            client,
            risk_manager,
            "BTCUSDT".into(),
            Decimal::from(1000),
            Decimal::from(10),
            Decimal::from(5),
            5000,
        );

        // Low confidence = 0.5x size
        let size = engine.calculate_position_size(0.0);
        assert_eq!(size, Decimal::from(500));

        // Medium confidence = 1.25x size
        let size = engine.calculate_position_size(0.5);
        assert_eq!(size, Decimal::from(1250));

        // High confidence = 2.0x size
        let size = engine.calculate_position_size(1.0);
        assert_eq!(size, Decimal::from(2000));
    }
}
