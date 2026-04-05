//! x402 v2 wire types (minimal subset for sellers).

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
}

/// Parse the `X-PAYMENT` header value into the JSON object pr402 expects for verify/settle.
pub fn parse_x_payment_header(raw: &str) -> Result<Value, XPaymentParseError> {
    Ok(serde_json::from_str(raw.trim())?)
}
