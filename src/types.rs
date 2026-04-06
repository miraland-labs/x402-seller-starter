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
pub enum XPaymentParseError {
    #[error("X-PAYMENT must be UTF-8")]
    Encoding,
    #[error("X-PAYMENT must be JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("X-PAYMENT is not JSON or valid base64 JSON: {0}")]
    Base64(String),
}

/// Parse the `X-PAYMENT` header value into the JSON object pr402 expects for verify/settle.
pub fn parse_x_payment_header(raw: &str) -> Result<Value, XPaymentParseError> {
    let trimmed = raw.trim();
    if let Ok(v) = serde_json::from_str(trimmed) {
        return Ok(v);
    }
    let bytes = B64
        .decode(trimmed)
        .map_err(|e| XPaymentParseError::Base64(e.to_string()))?;
    let s = String::from_utf8(bytes).map_err(|_| XPaymentParseError::Encoding)?;
    Ok(serde_json::from_str(&s)?)
}

#[cfg(test)]
mod tests {
    use super::parse_x_payment_header;
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    use serde_json::json;

    #[test]
    fn parses_raw_json() {
        let body = json!({"x402Version": 2, "paymentPayload": {}, "paymentRequirements": {}});
        let parsed = parse_x_payment_header(&body.to_string()).expect("raw JSON should parse");
        assert_eq!(parsed["x402Version"], 2);
    }

    #[test]
    fn parses_base64_json() {
        let body = json!({"x402Version": 2, "paymentPayload": {}, "paymentRequirements": {}});
        let encoded = B64.encode(body.to_string());
        let parsed = parse_x_payment_header(&encoded).expect("base64 JSON should parse");
        assert_eq!(parsed["x402Version"], 2);
    }
}
