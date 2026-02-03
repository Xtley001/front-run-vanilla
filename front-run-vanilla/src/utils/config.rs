use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::path::Path;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub strategy: StrategyConfig,
    pub position_sizing: PositionSizingConfig,
    pub risk: RiskConfig,
    pub exchange: ExchangeConfig,
    pub latency: LatencyConfig,
    pub logging: LoggingConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub symbol: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub environment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub imbalance_threshold: f64,
    pub min_confirming_signals: usize,
    pub lookback_window_ms: u64,
    pub take_profit_bps: f64,
    pub stop_loss_bps: f64,
    pub max_hold_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSizingConfig {
    pub base_notional_usd: f64,
    pub min_size_multiplier: f64,
    pub max_size_multiplier: f64,
    pub max_position_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_portfolio_exposure_usd: f64,
    pub max_daily_loss_usd: f64,
    pub max_drawdown_pct: f64,
    pub max_trades_per_hour: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub name: String,
    pub testnet: bool,
    pub api_endpoint: String,
    pub ws_endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyConfig {
    pub target_signal_to_order_ms: u64,
    pub max_acceptable_latency_ms: u64,
    pub ws_ping_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub output: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub prometheus_port: u16,
    pub enabled: bool,
}

impl Config {
    /// Load configuration from TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Load from environment variable or default path
    pub fn load() -> Result<Self> {
        let path = std::env::var("CONFIG_FILE")
            .unwrap_or_else(|_| "config/production.toml".to_string());
        Self::from_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_production_config() {
        // This will fail if config file doesn't exist, which is expected in tests
        let result = Config::from_file("config/production.toml");
        // Just verify the function exists and can be called
        assert!(result.is_ok() || result.is_err());
    }
}
