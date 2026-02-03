use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

/// Generate HMAC-SHA256 signature for Binance API requests
/// 
/// Binance requires all authenticated endpoints to include:
/// 1. timestamp parameter
/// 2. signature parameter (HMAC-SHA256 of query string)
pub fn generate_signature(secret_key: &str, query_string: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
        .expect("HMAC can take key of any size");
    
    mac.update(query_string.as_bytes());
    
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Get current timestamp in milliseconds
pub fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as u64
}

/// Build signed query string for Binance API
/// 
/// Example:
/// ```
/// let params = vec![
///     ("symbol", "BTCUSDT"),
///     ("side", "BUY"),
///     ("type", "MARKET"),
///     ("quantity", "0.001"),
/// ];
/// let query = build_signed_query(&params, "your_secret_key");
/// ```
pub fn build_signed_query(params: &[(&str, &str)], secret_key: &str) -> String {
    let timestamp = get_timestamp();
    
    // Build query string with timestamp
    let mut query_params: Vec<String> = params.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    
    query_params.push(format!("timestamp={}", timestamp));
    
    let query_string = query_params.join("&");
    
    // Generate signature
    let signature = generate_signature(secret_key, &query_string);
    
    // Add signature to query
    format!("{}&signature={}", query_string, signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_signature() {
        let secret = "test_secret_key";
        let query = "symbol=BTCUSDT&side=BUY&type=MARKET&quantity=0.001&timestamp=1234567890";
        
        let signature = generate_signature(secret, query);
        
        // Signature should be 64 character hex string
        assert_eq!(signature.len(), 64);
        
        // Same input should produce same signature
        let signature2 = generate_signature(secret, query);
        assert_eq!(signature, signature2);
    }

    #[test]
    fn test_build_signed_query() {
        let params = vec![
            ("symbol", "BTCUSDT"),
            ("side", "BUY"),
            ("type", "MARKET"),
            ("quantity", "0.001"),
        ];
        
        let query = build_signed_query(&params, "secret");
        
        // Should contain all parameters
        assert!(query.contains("symbol=BTCUSDT"));
        assert!(query.contains("side=BUY"));
        assert!(query.contains("type=MARKET"));
        assert!(query.contains("quantity=0.001"));
        assert!(query.contains("timestamp="));
        assert!(query.contains("signature="));
    }

    #[test]
    fn test_timestamp() {
        let ts1 = get_timestamp();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ts2 = get_timestamp();
        
        assert!(ts2 > ts1);
    }
}
