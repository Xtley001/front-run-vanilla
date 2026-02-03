use front_run_vanilla::{BacktestEngine, BacktestConfig, BacktestEvent};
use rust_decimal::Decimal;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use clap::Parser;

/// Backtest the trading strategy on historical data
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Start date (YYYY-MM-DD)
    #[arg(short, long, default_value = "2024-01-01")]
    start: String,

    /// End date (YYYY-MM-DD)
    #[arg(short, long, default_value = "2024-12-31")]
    end: String,

    /// Symbol to backtest
    #[arg(long, default_value = "BTCUSDT")]
    symbol: String,

    /// Initial capital
    #[arg(long, default_value = "10000")]
    capital: f64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("╔════════════════════════════════════════════════╗");
    println!("║         BACKTESTING ENGINE                     ║");
    println!("╚════════════════════════════════════════════════╝");
    println!();
    println!("Symbol: {}", args.symbol);
    println!("Period: {} to {}", args.start, args.end);
    println!("Initial Capital: ${}", args.capital);
    println!();

    // Create configuration
    let config = BacktestConfig {
        symbol: args.symbol.clone(),
        initial_capital: Decimal::from_f64_retain(args.capital).unwrap(),
        position_size: Decimal::from(1000),
        take_profit_bps: Decimal::from(10),
        stop_loss_bps: Decimal::from(5),
        max_hold_time_ms: 5000,
        slippage_bps: Decimal::from(2),
        commission_bps: Decimal::from(4),
        latency_ms: 100,
    };

    // Create backtesting engine
    let mut engine = BacktestEngine::new(config);

    println!("Loading historical data...");
    
    // NOTE: In production, you would load real historical data here
    // For this demo, we'll generate synthetic data
    let events = generate_synthetic_data(&args.symbol, &args.start, &args.end)?;
    
    println!("Loaded {} events", events.len());
    println!();
    println!("Running backtest...");

    // Process all events
    for (i, event) in events.iter().enumerate() {
        engine.process_event(event.clone())?;

        // Progress indicator
        if i % 10000 == 0 {
            print!(".");
            std::io::Write::flush(&mut std::io::stdout())?;
        }
    }

    println!();
    println!();

    // Get and print results
    let results = engine.get_results();
    results.print_summary();

    // Save results to JSON
    let json = serde_json::to_string_pretty(&results)?;
    std::fs::write("backtest_results.json", json)?;
    println!("Results saved to: backtest_results.json");

    Ok(())
}

/// Generate synthetic market data for backtesting demonstration
/// In production, replace this with actual historical data loading
fn generate_synthetic_data(
    symbol: &str,
    start: &str,
    end: &str,
) -> anyhow::Result<Vec<BacktestEvent>> {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    let mut events = Vec::new();
    let mut current_time = UNIX_EPOCH + Duration::from_secs(1704067200); // 2024-01-01
    let mut current_price = Decimal::from(100000); // Starting price

    // Generate 100,000 events (about 1 day of data at 100ms intervals)
    for _ in 0..100000 {
        // Random walk for price
        let change = rng.gen_range(-0.001..0.001);
        current_price = current_price * (Decimal::ONE + Decimal::from_f64_retain(change).unwrap());

        // Generate bid/ask levels
        let spread = current_price * Decimal::from_f64_retain(0.0001).unwrap();
        let mid = current_price;
        
        let mut bids = Vec::new();
        let mut asks = Vec::new();

        for i in 0..10 {
            let offset = Decimal::from(i) * spread;
            let bid_qty = Decimal::from_f64_retain(rng.gen_range(0.1..5.0)).unwrap();
            let ask_qty = Decimal::from_f64_retain(rng.gen_range(0.1..5.0)).unwrap();

            bids.push((mid - offset, bid_qty));
            asks.push((mid + offset, ask_qty));
        }

        events.push(BacktestEvent::OrderBookUpdate {
            timestamp: current_time,
            bids,
            asks,
        });

        // Occasionally add trades
        if rng.gen_bool(0.1) {
            let trade = crate::data::Trade {
                id: events.len() as u64,
                price: current_price,
                quantity: Decimal::from_f64_retain(rng.gen_range(0.01..0.5)).unwrap(),
                side: if rng.gen_bool(0.5) { 
                    crate::data::Side::Buy 
                } else { 
                    crate::data::Side::Sell 
                },
                timestamp: current_time,
                is_buyer_maker: rng.gen_bool(0.5),
            };

            events.push(BacktestEvent::Trade {
                timestamp: current_time,
                trade,
            });
        }

        current_time += Duration::from_millis(100);
    }

    Ok(events)
}

