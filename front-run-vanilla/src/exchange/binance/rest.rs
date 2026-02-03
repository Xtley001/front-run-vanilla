use crate::data::{Side, Order, OrderType};
use crate::exchange::binance::{auth, types::OrderResponse};
use anyhow::{Result, anyhow};
use reqwest::Client;
use rust_decimal::Decimal;
use std::time::Duration;
use tracing::{info, error};

/// Binance Futures REST API client
pub struct BinanceRestClient {
    client: Client,
    api_key: String,
    secret_key: String,
    base_url: String,
}

impl BinanceRestClient {
    /// Create new REST client
    pub fn new(api_key: String, secret_key: String, base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            secret_key,
            base_url,
        }
    }

    /// Place a market order
    /// 
    /// CRITICAL: This is the execution path with strict latency requirements
    pub async fn place_market_order(
        &self,
        symbol: &str,
        side: Side,
        quantity: Decimal,
    ) -> Result<OrderResponse> {
        let side_str = match side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        };

        let params = vec![
            ("symbol", symbol),
            ("side", side_str),
            ("type", "MARKET"),
            ("quantity", &quantity.to_string()),
        ];

        self.execute_signed_request("/fapi/v1/order", &params).await
    }

    /// Place a limit order
    pub async fn place_limit_order(
        &self,
        symbol: &str,
        side: Side,
        price: Decimal,
        quantity: Decimal,
    ) -> Result<OrderResponse> {
        let side_str = match side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        };

        let params = vec![
            ("symbol", symbol),
            ("side", side_str),
            ("type", "LIMIT"),
            ("timeInForce", "GTC"),  // Good-Till-Cancel
            ("price", &price.to_string()),
            ("quantity", &quantity.to_string()),
        ];

        self.execute_signed_request("/fapi/v1/order", &params).await
    }

    /// Cancel an order
    pub async fn cancel_order(&self, symbol: &str, order_id: u64) -> Result<OrderResponse> {
        let params = vec![
            ("symbol", symbol),
            ("orderId", &order_id.to_string()),
        ];

        let query_string = auth::build_signed_query(&params, &self.secret_key);
        let url = format!("{}/fapi/v1/order?{}", self.base_url, query_string);

        let response = self.client
            .delete(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Cancel order failed: {}", error_text));
        }

        let order_response = response.json::<OrderResponse>().await?;
        Ok(order_response)
    }

    /// Get account information
    pub async fn get_account_info(&self) -> Result<serde_json::Value> {
        let params = vec![];
        let query_string = auth::build_signed_query(&params, &self.secret_key);
        let url = format!("{}/fapi/v2/account?{}", self.base_url, query_string);

        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Get account info failed: {}", error_text));
        }

        let info = response.json::<serde_json::Value>().await?;
        Ok(info)
    }

    /// Execute signed POST request
    async fn execute_signed_request(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
    ) -> Result<OrderResponse> {
        let query_string = auth::build_signed_query(params, &self.secret_key);
        let url = format!("{}{}", self.base_url, endpoint);

        info!("Executing order: {} with params: {}", url, query_string);

        let response = self.client
            .post(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(query_string)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            error!("Order failed: {} - {}", status, error_text);
            return Err(anyhow!("Order execution failed: {} - {}", status, error_text));
        }

        let order_response = response.json::<OrderResponse>().await?;
        info!("Order executed successfully: {:?}", order_response);

        Ok(order_response)
    }

    /// Test connectivity to Binance API
    pub async fn test_connectivity(&self) -> Result<()> {
        let url = format!("{}/fapi/v1/ping", self.base_url);
        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!("Connectivity test failed"))
        }
    }

    /// Get exchange time (useful for time sync)
    pub async fn get_server_time(&self) -> Result<u64> {
        let url = format!("{}/fapi/v1/time", self.base_url);
        let response = self.client.get(&url).send().await?;

        #[derive(serde::Deserialize)]
        struct ServerTime {
            #[serde(rename = "serverTime")]
            server_time: u64,
        }

        let time = response.json::<ServerTime>().await?;
        Ok(time.server_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = BinanceRestClient::new(
            "test_api_key".to_string(),
            "test_secret_key".to_string(),
            "https://testnet.binancefuture.com".to_string(),
        );

        assert_eq!(client.api_key, "test_api_key");
        assert_eq!(client.base_url, "https://testnet.binancefuture.com");
    }

    // Note: Integration tests with real API should be in tests/ directory
    // and require valid credentials
}
