use front_run_vanilla::{
    OrderBook, BinanceWebSocket, MarketEvent,
    ImbalanceDetector, FlowAnalyzer, SignalAggregator,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    info!("Starting Front Run Vanilla - Paper Trading Mode");
    info!("============================================");

    // Configuration (from .env or config file in production)
    let symbol = "BTCUSDT".to_string();
    let ws_endpoint = "wss://fstream.binance.com".to_string();

    // Create shared order book
    let orderbook = Arc::new(OrderBook::new(&symbol));

    // Create WebSocket connection
    let (ws, mut event_rx) = BinanceWebSocket::new(
        symbol.clone(),
        ws_endpoint,
        Arc::clone(&orderbook),
    );

    // Start WebSocket in background
    let ws_handle = tokio::spawn(async move {
        ws.run().await;
    });

    // Create signal detectors
    let mut imbalance_detector = ImbalanceDetector::new(
        5,      // 5 levels
        100,    // 100 sample window
        3.0,    // 3.0 sigma threshold
    );

    let mut flow_analyzer = FlowAnalyzer::new(
        20,     // 20 trades window
        5000,   // 5 second time window
        0.6,    // 60% flow imbalance threshold
    );

    let signal_aggregator = SignalAggregator::new(
        3.0,    // Primary threshold
        1.5,    // Confirming threshold
        2,      // Min 2 confirming signals
    );

    // Stats tracking
    let mut event_count = 0;
    let mut signal_count = 0;
    let mut trade_signal_count = 0;

    info!("System initialized. Waiting for market data...");
    info!("");

    // Main event loop
    while let Some(event) = event_rx.recv().await {
        match event {
            MarketEvent::Connected => {
                info!("✓ Connected to Binance WebSocket");
            }

            MarketEvent::Disconnected => {
                warn!("✗ Disconnected from Binance WebSocket");
            }

            MarketEvent::DepthUpdate(_update) => {
                event_count += 1;

                // Every 10 updates, check for signals
                if event_count % 10 == 0 {
                    let mut signals = Vec::new();

                    // 1. Check imbalance signal
                    if let Some(signal) = imbalance_detector.calculate_signal(&orderbook) {
                        info!(
                            "📊 Imbalance Signal: {:?} | Strength: {:.2} | Confidence: {:.2}",
                            signal.direction, signal.strength, signal.confidence
                        );
                        signals.push(signal);
                        signal_count += 1;
                    }

                    // 2. Aggregate signals
                    if !signals.is_empty() {
                        if let Some(composite) = signal_aggregator.aggregate(signals) {
                            info!("");
                            info!("🎯 COMPOSITE SIGNAL GENERATED");
                            info!("   Direction: {:?}", composite.direction);
                            info!("   Strength: {:.2}", composite.overall_strength);
                            info!("   Confidence: {:.2}", composite.confidence);
                            info!("   Confirming: {}", composite.confirming.len());
                            
                            if composite.is_tradeable(2) {
                                info!("   ✅ TRADEABLE - Would execute in live mode");
                                trade_signal_count += 1;
                                
                                // In live mode, this is where we'd execute:
                                // execute_trade(composite).await;
                            } else {
                                info!("   ⚠ Not tradeable - insufficient confirming signals");
                            }
                            info!("");
                        }
                    }

                    // Print stats every 100 updates
                    if event_count % 100 == 0 {
                        let (best_bid, best_ask) = orderbook.get_top_of_book();
                        let mid = orderbook.get_mid_price();
                        let spread = orderbook.get_spread_bps();
                        let (bid_count, ask_count) = orderbook.get_book_depth_count();

                        info!("📈 Market Stats (after {} updates):", event_count);
                        info!("   Best Bid: {:?}", best_bid);
                        info!("   Best Ask: {:?}", best_ask);
                        info!("   Mid Price: {:?}", mid);
                        info!("   Spread: {:?} bps", spread);
                        info!("   Book Depth: {} bids, {} asks", bid_count, ask_count);
                        info!("   Signals: {} generated, {} tradeable", signal_count, trade_signal_count);
                        info!("");
                    }
                }
            }

            MarketEvent::Trade(trade) => {
                // Process trade for flow analysis
                if let Some(signal) = flow_analyzer.process_trade(trade) {
                    info!(
                        "💹 Flow Signal: {:?} | Strength: {:.2} | Confidence: {:.2}",
                        signal.direction, signal.strength, signal.confidence
                    );
                    signal_count += 1;
                }
            }
        }
    }

    // Wait for WebSocket task to complete (it won't, but handle shutdown gracefully)
    ws_handle.await?;

    Ok(())
}
