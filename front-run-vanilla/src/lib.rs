pub mod data;
pub mod exchange;
pub mod strategy;
pub mod risk;
pub mod backtest;
pub mod utils;

// Re-export commonly used types
pub use data::{OrderBook, Side, PriceLevel, Trade, Order, Signal, SignalComponent};
pub use exchange::{BinanceWebSocket, BinanceRestClient, MarketEvent};
pub use strategy::{
    ImbalanceDetector, FlowAnalyzer, SignalAggregator, CompositeSignal,
    ExecutionEngine, TradingStats,
};
pub use risk::{Position, PositionManager, RiskManager, RiskLimits};
pub use backtest::{BacktestEngine, BacktestConfig, BacktestResults};
pub use utils::Config;
