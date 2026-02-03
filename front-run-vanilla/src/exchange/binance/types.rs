use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Binance WebSocket depth update message
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DepthUpdate {
    #[serde(rename = "e")]
    pub event_type: String,  // "depthUpdate"
    
    #[serde(rename = "E")]
    pub event_time: u64,  // Event timestamp
    
    #[serde(rename = "s")]
    pub symbol: String,  // "BTCUSDT"
    
    #[serde(rename = "U")]
    pub first_update_id: u64,
    
    #[serde(rename = "u")]
    pub final_update_id: u64,
    
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,  // [["price", "quantity"], ...]
    
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,  // [["price", "quantity"], ...]
}

impl DepthUpdate {
    /// Parse bid levels into Decimal tuples
    pub fn parse_bids(&self) -> Vec<(Decimal, Decimal)> {
        self.bids.iter()
            .filter_map(|level| {
                let price = level[0].parse::<Decimal>().ok()?;
                let qty = level[1].parse::<Decimal>().ok()?;
                Some((price, qty))
            })
            .collect()
    }

    /// Parse ask levels into Decimal tuples
    pub fn parse_asks(&self) -> Vec<(Decimal, Decimal)> {
        self.asks.iter()
            .filter_map(|level| {
                let price = level[0].parse::<Decimal>().ok()?;
                let qty = level[1].parse::<Decimal>().ok()?;
                Some((price, qty))
            })
            .collect()
    }
}

/// Binance aggregated trade message
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AggTrade {
    #[serde(rename = "e")]
    pub event_type: String,  // "aggTrade"
    
    #[serde(rename = "E")]
    pub event_time: u64,
    
    #[serde(rename = "s")]
    pub symbol: String,
    
    #[serde(rename = "a")]
    pub agg_trade_id: u64,
    
    #[serde(rename = "p")]
    pub price: String,  // Price as string
    
    #[serde(rename = "q")]
    pub quantity: String,  // Quantity as string
    
    #[serde(rename = "f")]
    pub first_trade_id: u64,
    
    #[serde(rename = "l")]
    pub last_trade_id: u64,
    
    #[serde(rename = "T")]
    pub trade_time: u64,
    
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,  // true = seller is taker (sell aggression)
}

impl AggTrade {
    /// Convert to our Trade type
    pub fn to_trade(&self) -> Option<crate::data::types::Trade> {
        use crate::data::types::{Trade, Side};
        use std::time::{SystemTime, UNIX_EPOCH, Duration};

        let price = self.price.parse::<Decimal>().ok()?;
        let quantity = self.quantity.parse::<Decimal>().ok()?;
        
        // Determine taker side
        let side = if self.is_buyer_maker {
            Side::Sell  // Seller is taker (aggressive sell)
        } else {
            Side::Buy   // Buyer is taker (aggressive buy)
        };

        let timestamp = UNIX_EPOCH + Duration::from_millis(self.trade_time);

        Some(Trade {
            id: self.agg_trade_id,
            price,
            quantity,
            side,
            timestamp,
            is_buyer_maker: self.is_buyer_maker,
        })
    }
}

/// Binance WebSocket message wrapper
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BinanceMessage {
    DepthUpdate(DepthUpdate),
    AggTrade(AggTrade),
}

/// Order response from REST API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderResponse {
    #[serde(rename = "orderId")]
    pub order_id: u64,
    
    pub symbol: String,
    
    #[serde(rename = "clientOrderId")]
    pub client_order_id: String,
    
    pub price: String,
    
    #[serde(rename = "origQty")]
    pub orig_qty: String,
    
    #[serde(rename = "executedQty")]
    pub executed_qty: String,
    
    pub status: String,  // "NEW", "FILLED", etc.
    
    #[serde(rename = "timeInForce")]
    pub time_in_force: String,
    
    #[serde(rename = "type")]
    pub order_type: String,
    
    pub side: String,  // "BUY" or "SELL"
    
    #[serde(rename = "updateTime")]
    pub update_time: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_depth_update() {
        let json = r#"{
            "e": "depthUpdate",
            "E": 1234567890,
            "s": "BTCUSDT",
            "U": 1,
            "u": 2,
            "b": [["100.00", "1.5"], ["99.50", "2.0"]],
            "a": [["101.00", "1.0"], ["101.50", "0.5"]]
        }"#;

        let update: DepthUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update.symbol, "BTCUSDT");
        
        let bids = update.parse_bids();
        assert_eq!(bids.len(), 2);
        
        let asks = update.parse_asks();
        assert_eq!(asks.len(), 2);
    }

    #[test]
    fn test_parse_agg_trade() {
        let json = r#"{
            "e": "aggTrade",
            "E": 1234567890,
            "s": "BTCUSDT",
            "a": 12345,
            "p": "100.00",
            "q": "1.5",
            "f": 100,
            "l": 105,
            "T": 1234567890,
            "m": false
        }"#;

        let agg_trade: AggTrade = serde_json::from_str(json).unwrap();
        let trade = agg_trade.to_trade().unwrap();
        
        assert_eq!(trade.side, crate::data::types::Side::Buy);
        assert!(!trade.is_buyer_maker);
    }
}
