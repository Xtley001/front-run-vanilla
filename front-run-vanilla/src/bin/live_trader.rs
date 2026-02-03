use front_run_vanilla::{
    OrderBook, BinanceWebSocket, BinanceRestClient, MarketEvent,
    ImbalanceDetector, FlowAnalyzer, SignalAggregator,
    ExecutionEngine, RiskManager, RiskLimits, Config,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    info!("╔════════════════════════════════════════════════╗");
    info!("║   Front Run Vanilla - LIVE TRADING MODE       ║");
    info!("╠════════════════════════════════════════════════╣");
    info!("║   ⚠️  WARNING: REAL MONEY AT RISK!            ║");
    info!("║   Make sure you understand what you're doing   ║");
    info!("╚════════════════════════════════════════════════╝");
    info!("");

    // Load configuration
    let config = Config::load()?;
    info!("✓ Configuration loaded: {}", config.general.environment);

    // Get API credentials
    let api_key = env::var("BINANCE_API_KEY")
        .expect("BINANCE_API_KEY not found in environment");
    let secret_key = env::var("BINANCE_SECRET_KEY")
        .expect("BINANCE_SECRET_KEY not found in environment");

    info!("✓ API credentials loaded");

    // Create shared order book
    let orderbook = Arc::new(OrderBook::new(&config.general.symbol));

    // Create WebSocket connection
    let (ws, mut event_rx) = BinanceWebSocket::new(
        config.general.symbol.clone(),
        config.exchange.ws_endpoint.clone(),
        Arc::clone(&orderbook),
    );

    // Start WebSocket in background
    tokio::spawn(async move {
        ws.run().await;
    });

    // Create REST client for order execution
    let rest_client = BinanceRestClient::new(
        api_key,
        secret_key,
        config.exchange.api_endpoint.clone(),
    );

    // Test connectivity
    rest_client.test_connectivity().await?;
    info!("✓ Connected to Binance API");

    // Create signal detectors
    let mut imbalance_detector = ImbalanceDetector::new(
        5,
        100,
        config.strategy.imbalance_threshold,
    );

    let mut flow_analyzer = FlowAnalyzer::new(
        20,
        5000,
        0.6,
    );

    let signal_aggregator = SignalAggregator::new(
        config.strategy.imbalance_threshold,
        1.5,
        config.strategy.min_confirming_signals,
    );

    // Create risk manager
    let risk_limits = RiskLimits {
        max_position_size: Decimal::from_f64_retain(config.position_sizing.max_position_usd).unwrap(),
        max_portfolio_exposure: Decimal::from_f64_retain(config.risk.max_portfolio_exposure_usd).unwrap(),
        max_daily_loss: Decimal::from_f64_retain(config.risk.max_daily_loss_usd).unwrap(),
        max_drawdown_percent: Decimal::from_f64_retain(config.risk.max_drawdown_pct).unwrap(),
        max_trades_per_hour: config.risk.max_trades_per_hour,
        max_trades_per_day: 200,
        max_acceptable_latency_ms: config.latency.max_acceptable_latency_ms,
    };

    let risk_manager = RiskManager::new(
        risk_limits,
        Decimal::from_f64_retain(config.risk.max_portfolio_exposure_usd).unwrap(),
    );

    // Create execution engine
    let mut execution_engine = ExecutionEngine::new(
        rest_client,
        risk_manager,
        config.general.symbol.clone(),
        Decimal::from_f64_retain(config.position_sizing.base_notional_usd).unwrap(),
        Decimal::from_f64_retain(config.strategy.take_profit_bps).unwrap(),
        Decimal::from_f64_retain(config.strategy.stop_loss_bps).unwrap(),
        config.strategy.max_hold_time_ms,
    );

    info!("✓ Trading engine initialized");
    info!("");
    info!("System ready. Monitoring market for signals...");
    info!("Press Ctrl+C to stop");
    info!("");

    let mut event_count = 0;

    // Main trading loop
    while let Some(event) = event_rx.recv().await {
        match event {
            MarketEvent::Connected => {
                info!("✓ WebSocket connected");
            }

            MarketEvent::Disconnected => {
                warn!("✗ WebSocket disconnected");
            }

            MarketEvent::DepthUpdate(_) => {
                event_count += 1;

                // Check signals every 10 updates (~1 second)
                if event_count % 10 == 0 {
                    // Check for exit conditions first
                    if let Some(current_price) = orderbook.get_mid_price() {
                        if let Err(e) = execution_engine.check_exits(current_price).await {
                            error!("Error checking exits: {}", e);
                        }
                    }

                    // Check for entry signals
                    let mut signals = Vec::new();

                    if let Some(signal) = imbalance_detector.calculate_signal(&orderbook) {
                        info!("📊 Imbalance signal: {:?} | Strength: {:.2}", 
                            signal.direction, signal.strength);
                        signals.push(signal);
                    }

                    // Aggregate and execute if tradeable
                    if !signals.is_empty() {
                        if let Some(composite) = signal_aggregator.aggregate(signals) {
                            if composite.is_tradeable(config.strategy.min_confirming_signals) {
                                info!("");
                                info!("🎯 COMPOSITE SIGNAL GENERATED");
                                info!("   Direction: {:?}", composite.direction);
                                info!("   Confidence: {:.2}", composite.confidence);
                                
                                // Check if not halted
                                if execution_engine.risk_manager().is_halted() {
                                    warn!("   ⚠️  Trading halted: {}", 
                                        execution_engine.risk_manager().halt_reason().unwrap_or("Unknown"));
                                } else if let Some(current_price) = orderbook.get_mid_price() {
                                    info!("   Executing trade...");
                                    
                                    match execution_engine.execute_signal(composite, current_price).await {
                                        Ok(result) => {
                                            info!("   ✅ TRADE EXECUTED");
                                            info!("      Order ID: {}", result.order_id);
                                            info!("      Price: {}", result.executed_price);
                                            info!("      Quantity: {}", result.executed_qty);
                                            info!("      Latency: {}ms", result.latency_ms);
                                        }
                                        Err(e) => {
                                            error!("   ✗ Execution failed: {}", e);
                                        }
                                    }
                                }
                                info!("");
                            }
                        }
                    }

                    // Print stats every 1000 updates (~100 seconds)
                    if event_count % 1000 == 0 {
                        let stats = execution_engine.get_stats();
                        info!("📈 Trading Stats:");
                        info!("   Open Positions: {}", stats.open_positions);
                        info!("   Closed Trades: {}", stats.closed_trades);
                        info!("   Realized PnL: {}", stats.total_realized_pnl);
                        info!("   Win Rate: {:.2}%", stats.win_rate * 100.0);
                        info!("   Total Fees: {}", stats.total_fees);
                        info!("");
                    }
                }
            }

            MarketEvent::Trade(trade) => {
                if let Some(_signal) = flow_analyzer.process_trade(trade) {
                    // Flow signals are captured in the aggregate above
                }
            }
        }
    }

    Ok(())
}

