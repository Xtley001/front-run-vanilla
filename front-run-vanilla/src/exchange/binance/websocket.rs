use crate::data::{OrderBook, Trade};
use crate::exchange::binance::types::{BinanceMessage, DepthUpdate, AggTrade};
use anyhow::{Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn, error, debug};
use std::sync::Arc;
use std::time::Duration;

/// Events emitted by the WebSocket stream
#[derive(Debug, Clone)]
pub enum MarketEvent {
    DepthUpdate(DepthUpdate),
    Trade(Trade),
    Connected,
    Disconnected,
}

/// WebSocket connection manager with auto-reconnect
pub struct BinanceWebSocket {
    symbol: String,
    ws_url: String,
    event_tx: mpsc::UnboundedSender<MarketEvent>,
    orderbook: Arc<OrderBook>,
}

impl BinanceWebSocket {
    /// Create new WebSocket manager
    /// 
    /// Streams:
    /// - {symbol}@depth@100ms - Order book updates every 100ms
    /// - {symbol}@aggTrade - Aggregated trades
    pub fn new(
        symbol: String,
        ws_endpoint: String,
        orderbook: Arc<OrderBook>,
    ) -> (Self, mpsc::UnboundedReceiver<MarketEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        // Build WebSocket URL with combined streams
        let symbol_lower = symbol.to_lowercase();
        let streams = format!("{}@depth@100ms/{}@aggTrade", symbol_lower, symbol_lower);
        let ws_url = format!("{}/stream?streams={}", ws_endpoint, streams);

        (
            Self {
                symbol,
                ws_url,
                event_tx,
                orderbook,
            },
            event_rx,
        )
    }

    /// Start WebSocket connection with auto-reconnect
    /// 
    /// This runs indefinitely, automatically reconnecting on errors.
    /// Use tokio::spawn to run in background.
    pub async fn run(&self) {
        let mut reconnect_delay = Duration::from_secs(1);
        let max_reconnect_delay = Duration::from_secs(60);

        loop {
            info!("Connecting to Binance WebSocket: {}", self.ws_url);

            match self.connect_and_process().await {
                Ok(_) => {
                    info!("WebSocket connection closed normally");
                    reconnect_delay = Duration::from_secs(1);
                }
                Err(e) => {
                    error!("WebSocket error: {}. Reconnecting in {:?}", e, reconnect_delay);
                    
                    let _ = self.event_tx.send(MarketEvent::Disconnected);
                    
                    tokio::time::sleep(reconnect_delay).await;
                    
                    // Exponential backoff
                    reconnect_delay = std::cmp::min(
                        reconnect_delay * 2,
                        max_reconnect_delay,
                    );
                }
            }
        }
    }

    /// Connect and process messages
    async fn connect_and_process(&self) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.ws_url).await
            .map_err(|e| anyhow!("WebSocket connection failed: {}", e))?;

        info!("WebSocket connected successfully");
        let _ = self.event_tx.send(MarketEvent::Connected);

        let (mut write, mut read) = ws_stream.split();

        // Spawn ping task to keep connection alive
        let ping_interval = Duration::from_secs(30);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(ping_interval);
            loop {
                interval.tick().await;
                if write.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
        });

        // Process incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Err(e) = self.process_message(&text).await {
                        warn!("Error processing message: {}", e);
                    }
                }
                Ok(Message::Ping(_)) => {
                    debug!("Received ping");
                }
                Ok(Message::Pong(_)) => {
                    debug!("Received pong");
                }
                Ok(Message::Close(_)) => {
                    info!("Received close frame");
                    break;
                }
                Err(e) => {
                    return Err(anyhow!("WebSocket error: {}", e));
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Process a single WebSocket message
    async fn process_message(&self, text: &str) -> Result<()> {
        // Binance streams come wrapped in {"stream": "...", "data": {...}}
        #[derive(serde::Deserialize)]
        struct StreamWrapper {
            stream: String,
            data: serde_json::Value,
        }

        let wrapper: StreamWrapper = serde_json::from_str(text)
            .map_err(|e| anyhow!("Failed to parse stream wrapper: {}", e))?;

        // Determine message type from stream name
        if wrapper.stream.contains("depth") {
            self.process_depth_update(&wrapper.data).await?;
        } else if wrapper.stream.contains("aggTrade") {
            self.process_agg_trade(&wrapper.data).await?;
        }

        Ok(())
    }

    /// Process depth update and update order book
    async fn process_depth_update(&self, data: &serde_json::Value) -> Result<()> {
        let update: DepthUpdate = serde_json::from_value(data.clone())
            .map_err(|e| anyhow!("Failed to parse depth update: {}", e))?;

        // Update order book with bids
        for (price, qty) in update.parse_bids() {
            self.orderbook.update_level(crate::data::Side::Buy, price, qty)?;
        }

        // Update order book with asks
        for (price, qty) in update.parse_asks() {
            self.orderbook.update_level(crate::data::Side::Sell, price, qty)?;
        }

        // Send event
        let _ = self.event_tx.send(MarketEvent::DepthUpdate(update));

        Ok(())
    }

    /// Process aggregated trade
    async fn process_agg_trade(&self, data: &serde_json::Value) -> Result<()> {
        let agg_trade: AggTrade = serde_json::from_value(data.clone())
            .map_err(|e| anyhow!("Failed to parse agg trade: {}", e))?;

        if let Some(trade) = agg_trade.to_trade() {
            let _ = self.event_tx.send(MarketEvent::Trade(trade));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_stream_wrapper() {
        let json = r#"{
            "stream": "btcusdt@depth@100ms",
            "data": {
                "e": "depthUpdate",
                "E": 1234567890,
                "s": "BTCUSDT",
                "U": 1,
                "u": 2,
                "b": [["100.00", "1.5"]],
                "a": [["101.00", "1.0"]]
            }
        }"#;

        #[derive(serde::Deserialize)]
        struct StreamWrapper {
            stream: String,
            data: serde_json::Value,
        }

        let wrapper: StreamWrapper = serde_json::from_str(json).unwrap();
        assert_eq!(wrapper.stream, "btcusdt@depth@100ms");
        
        let update: DepthUpdate = serde_json::from_value(wrapper.data).unwrap();
        assert_eq!(update.symbol, "BTCUSDT");
    }
}
