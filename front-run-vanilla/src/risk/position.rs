use crate::data::{Side, Order};
use rust_decimal::Decimal;
use std::time::{SystemTime, Duration};
use serde::{Serialize, Deserialize};
use anyhow::Result;

/// Position tracker with real-time PnL calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: Side,  // Long (Buy) or Short (Sell)
    pub entry_price: Decimal,
    pub quantity: Decimal,
    pub entry_time: SystemTime,
    pub realized_pnl: Decimal,
    pub fees_paid: Decimal,
}

impl Position {
    /// Create a new position
    pub fn new(
        symbol: String,
        side: Side,
        entry_price: Decimal,
        quantity: Decimal,
        fees: Decimal,
    ) -> Self {
        Self {
            symbol,
            side,
            entry_price,
            quantity,
            entry_time: SystemTime::now(),
            realized_pnl: Decimal::ZERO,
            fees_paid: fees,
        }
    }

    /// Calculate unrealized PnL at current price
    pub fn unrealized_pnl(&self, current_price: Decimal) -> Decimal {
        let price_diff = match self.side {
            Side::Buy => current_price - self.entry_price,  // Long: profit if price up
            Side::Sell => self.entry_price - current_price, // Short: profit if price down
        };

        price_diff * self.quantity - self.fees_paid
    }

    /// Calculate unrealized PnL percentage
    pub fn unrealized_pnl_percent(&self, current_price: Decimal) -> Decimal {
        let pnl = self.unrealized_pnl(current_price);
        let cost_basis = self.entry_price * self.quantity;
        
        if cost_basis.is_zero() {
            return Decimal::ZERO;
        }

        (pnl / cost_basis) * Decimal::from(100)
    }

    /// Close position and calculate realized PnL
    pub fn close(&mut self, exit_price: Decimal, exit_fees: Decimal) -> Decimal {
        let pnl = self.unrealized_pnl(exit_price) - exit_fees;
        self.realized_pnl = pnl;
        self.fees_paid += exit_fees;
        pnl
    }

    /// Get position notional value
    pub fn notional_value(&self) -> Decimal {
        self.entry_price * self.quantity
    }

    /// Get position age
    pub fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.entry_time)
            .unwrap_or(Duration::ZERO)
    }

    /// Check if position has been open too long
    pub fn is_expired(&self, max_hold_time_ms: u64) -> bool {
        self.age().as_millis() as u64 > max_hold_time_ms
    }

    /// Check if take profit hit
    pub fn take_profit_hit(&self, current_price: Decimal, take_profit_bps: Decimal) -> bool {
        let pnl_pct = self.unrealized_pnl_percent(current_price);
        let target = take_profit_bps / Decimal::from(100); // Convert bps to percent
        pnl_pct >= target
    }

    /// Check if stop loss hit
    pub fn stop_loss_hit(&self, current_price: Decimal, stop_loss_bps: Decimal) -> bool {
        let pnl_pct = self.unrealized_pnl_percent(current_price);
        let target = -(stop_loss_bps / Decimal::from(100)); // Negative for loss
        pnl_pct <= target
    }
}

/// Position manager tracking all open positions
pub struct PositionManager {
    positions: Vec<Position>,
    closed_positions: Vec<Position>,
    total_realized_pnl: Decimal,
    total_fees: Decimal,
}

impl PositionManager {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            closed_positions: Vec::new(),
            total_realized_pnl: Decimal::ZERO,
            total_fees: Decimal::ZERO,
        }
    }

    /// Open a new position
    pub fn open_position(&mut self, position: Position) -> Result<()> {
        self.positions.push(position);
        Ok(())
    }

    /// Close a position
    pub fn close_position(
        &mut self,
        symbol: &str,
        exit_price: Decimal,
        exit_fees: Decimal,
    ) -> Result<Decimal> {
        let pos_idx = self.positions.iter()
            .position(|p| p.symbol == symbol)
            .ok_or_else(|| anyhow::anyhow!("Position not found: {}", symbol))?;

        let mut position = self.positions.remove(pos_idx);
        let realized_pnl = position.close(exit_price, exit_fees);

        self.total_realized_pnl += realized_pnl;
        self.total_fees += position.fees_paid;
        self.closed_positions.push(position);

        Ok(realized_pnl)
    }

    /// Get all open positions
    pub fn open_positions(&self) -> &[Position] {
        &self.positions
    }

    /// Get position for symbol
    pub fn get_position(&self, symbol: &str) -> Option<&Position> {
        self.positions.iter().find(|p| p.symbol == symbol)
    }

    /// Get total unrealized PnL across all positions
    pub fn total_unrealized_pnl(&self, prices: &[(String, Decimal)]) -> Decimal {
        self.positions.iter()
            .filter_map(|pos| {
                prices.iter()
                    .find(|(sym, _)| sym == &pos.symbol)
                    .map(|(_, price)| pos.unrealized_pnl(*price))
            })
            .sum()
    }

    /// Get total exposure (notional value of all positions)
    pub fn total_exposure(&self) -> Decimal {
        self.positions.iter()
            .map(|p| p.notional_value())
            .sum()
    }

    /// Get position count
    pub fn position_count(&self) -> usize {
        self.positions.len()
    }

    /// Get total realized PnL
    pub fn total_realized_pnl(&self) -> Decimal {
        self.total_realized_pnl
    }

    /// Get total fees paid
    pub fn total_fees(&self) -> Decimal {
        self.total_fees
    }

    /// Get closed positions for analysis
    pub fn closed_positions(&self) -> &[Position] {
        &self.closed_positions
    }

    /// Calculate win rate
    pub fn win_rate(&self) -> f64 {
        if self.closed_positions.is_empty() {
            return 0.0;
        }

        let wins = self.closed_positions.iter()
            .filter(|p| p.realized_pnl > Decimal::ZERO)
            .count();

        wins as f64 / self.closed_positions.len() as f64
    }

    /// Get average trade PnL
    pub fn average_trade_pnl(&self) -> Decimal {
        if self.closed_positions.is_empty() {
            return Decimal::ZERO;
        }

        self.total_realized_pnl / Decimal::from(self.closed_positions.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_long_position_profit() {
        let pos = Position::new(
            "BTCUSDT".to_string(),
            Side::Buy,
            dec!(100.0),
            dec!(1.0),
            dec!(0.04), // 0.04% fee
        );

        // Price goes up 10%
        let pnl = pos.unrealized_pnl(dec!(110.0));
        assert!(pnl > Decimal::ZERO);
        assert_eq!(pnl, dec!(9.96)); // 10 - 0.04 fee
    }

    #[test]
    fn test_long_position_loss() {
        let pos = Position::new(
            "BTCUSDT".to_string(),
            Side::Buy,
            dec!(100.0),
            dec!(1.0),
            dec!(0.04),
        );

        // Price goes down 5%
        let pnl = pos.unrealized_pnl(dec!(95.0));
        assert!(pnl < Decimal::ZERO);
        assert_eq!(pnl, dec!(-5.04)); // -5 - 0.04 fee
    }

    #[test]
    fn test_short_position_profit() {
        let pos = Position::new(
            "BTCUSDT".to_string(),
            Side::Sell,
            dec!(100.0),
            dec!(1.0),
            dec!(0.04),
        );

        // Price goes down 10% (profit for short)
        let pnl = pos.unrealized_pnl(dec!(90.0));
        assert!(pnl > Decimal::ZERO);
        assert_eq!(pnl, dec!(9.96));
    }

    #[test]
    fn test_take_profit_hit() {
        let pos = Position::new(
            "BTCUSDT".to_string(),
            Side::Buy,
            dec!(100.0),
            dec!(1.0),
            dec!(0.0),
        );

        // Take profit at 10 bps (0.10%)
        assert!(!pos.take_profit_hit(dec!(100.05), dec!(10.0))); // Only 0.05%
        assert!(pos.take_profit_hit(dec!(100.15), dec!(10.0)));  // 0.15% hit!
    }

    #[test]
    fn test_stop_loss_hit() {
        let pos = Position::new(
            "BTCUSDT".to_string(),
            Side::Buy,
            dec!(100.0),
            dec!(1.0),
            dec!(0.0),
        );

        // Stop loss at 5 bps (0.05%)
        assert!(!pos.stop_loss_hit(dec!(99.97), dec!(5.0))); // Only -0.03%
        assert!(pos.stop_loss_hit(dec!(99.93), dec!(5.0)));  // -0.07% hit!
    }

    #[test]
    fn test_position_manager() {
        let mut manager = PositionManager::new();

        let pos1 = Position::new(
            "BTCUSDT".to_string(),
            Side::Buy,
            dec!(100.0),
            dec!(1.0),
            dec!(0.04),
        );

        manager.open_position(pos1).unwrap();
        assert_eq!(manager.position_count(), 1);

        let pnl = manager.close_position("BTCUSDT", dec!(110.0), dec!(0.04)).unwrap();
        assert_eq!(pnl, dec!(9.92)); // 10 - 0.04 - 0.04
        assert_eq!(manager.position_count(), 0);
        assert_eq!(manager.closed_positions().len(), 1);
    }

    #[test]
    fn test_win_rate() {
        let mut manager = PositionManager::new();

        // Win
        let mut pos1 = Position::new("BTC".into(), Side::Buy, dec!(100.0), dec!(1.0), dec!(0.0));
        pos1.close(dec!(110.0), dec!(0.0));
        manager.closed_positions.push(pos1);

        // Loss
        let mut pos2 = Position::new("BTC".into(), Side::Buy, dec!(100.0), dec!(1.0), dec!(0.0));
        pos2.close(dec!(95.0), dec!(0.0));
        manager.closed_positions.push(pos2);

        // Win
        let mut pos3 = Position::new("BTC".into(), Side::Buy, dec!(100.0), dec!(1.0), dec!(0.0));
        pos3.close(dec!(105.0), dec!(0.0));
        manager.closed_positions.push(pos3);

        let win_rate = manager.win_rate();
        assert!((win_rate - 0.666).abs() < 0.01); // 2/3 = 66.6%
    }
}
