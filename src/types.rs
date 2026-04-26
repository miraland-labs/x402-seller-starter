//! x402 v2 wire types (minimal subset for sellers).

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_extensions_object() -> Value {
    serde_json::json!({})
}

/// §5.1 resource block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    pub url: String,
    pub description: String,
    pub mime_type: String,
}

/// HTTP 402 JSON body (x402 v2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequired {
    pub x402_version: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub resource: ResourceInfo,
    pub accepts: Vec<Value>,
    #[serde(default = "default_extensions_object")]
    pub extensions: Value,
}

impl PaymentRequired {
    pub fn with_error(mut self, message: impl Into<String>) -> Self {
        self.error = Some(message.into());
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PaymentParseError {
    #[error("payment header must be UTF-8")]
    Encoding,
    #[error("payment header must be JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("payment header is not JSON or valid base64 JSON: {0}")]
    Base64(String),
}

/// Parse the `PAYMENT-SIGNATURE` (x402 v2) header value
/// into the JSON object pr402 expects for verify/settle.
/// Accepts both raw JSON and base64-encoded JSON.
pub fn parse_payment_header(raw: &str) -> Result<Value, PaymentParseError> {
    let trimmed = raw.trim();
    if let Ok(v) = serde_json::from_str(trimmed) {
        return Ok(v);
    }
    let bytes = B64
        .decode(trimmed)
        .map_err(|e| PaymentParseError::Base64(e.to_string()))?;
    let s = String::from_utf8(bytes).map_err(|_| PaymentParseError::Encoding)?;
    Ok(serde_json::from_str(&s)?)
}

/// Extract payment proof: `PAYMENT-SIGNATURE` only (x402 v2).
pub fn extract_payment_header_value(get_header: impl Fn(&str) -> Option<String>) -> Option<String> {
    get_header("PAYMENT-SIGNATURE")
}

/// Encode a settlement result as a base64 string for the `PAYMENT-RESPONSE` header.
pub fn encode_payment_response(settle_result: &Value) -> String {
    B64.encode(settle_result.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    use serde_json::json;

    #[test]
    fn parses_raw_json() {
        let body = json!({"x402Version": 2, "paymentPayload": {}, "paymentRequirements": {}});
        let parsed = parse_payment_header(&body.to_string()).expect("raw JSON should parse");
        assert_eq!(parsed["x402Version"], 2);
    }

    #[test]
    fn parses_base64_json() {
        let body = json!({"x402Version": 2, "paymentPayload": {}, "paymentRequirements": {}});
        let encoded = B64.encode(body.to_string());
        let parsed = parse_payment_header(&encoded).expect("base64 JSON should parse");
        assert_eq!(parsed["x402Version"], 2);
    }

    #[test]
    fn extract_reads_payment_signature() {
        let result = extract_payment_header_value(|name| match name {
            "PAYMENT-SIGNATURE" => Some("proof".into()),
            _ => None,
        });
        assert_eq!(result.as_deref(), Some("proof"));
    }

    #[test]
    fn extract_missing_without_payment_signature() {
        let result = extract_payment_header_value(|name| match name {
            "X-PAYMENT" => Some("ignored".into()),
            _ => None,
        });
        assert_eq!(result, None);
    }

    #[test]
    fn encode_payment_response_roundtrip() {
        let settle = json!({"success": true, "payer": "abc", "network": "solana:devnet"});
        let encoded = encode_payment_response(&settle);
        let decoded: Value = serde_json::from_slice(&B64.decode(&encoded).unwrap()).unwrap();
        assert_eq!(decoded["success"], true);
        assert_eq!(decoded["payer"], "abc");
    }
}
