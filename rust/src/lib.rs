//! Reference **seller-side** helpers for [x402 v2](https://github.com/coinbase/x402/blob/main/specs/x402-specification-v2.md)
//! responses compatible with the [pr402](https://github.com/miralandlabs/pr402) facilitator.
//!
//! - Build [`PaymentRequired`](crate::PaymentRequired) JSON from environment or a JSON blob.
//! - [`exact_kind_extra_from_supported`](crate::exact_kind_extra_from_supported) to pull `extra` from facilitator **`GET /supported`**.
//! - Optional [`FacilitatorClient`] for verify + settle when the buyer sends `PAYMENT-SIGNATURE`
//!   (x402 v2; treats common duplicate on-chain settle errors as success after a valid verify).
//! - [`extract_payment_header_value`] reads `PAYMENT-SIGNATURE` only.
//! - [`encode_payment_response`] encodes a settlement result for the `PAYMENT-RESPONSE` header.
//!
//! Run the example server: `cargo run --example axum_server`

mod accepts;
mod facilitator;
mod sla_escrow;
mod srm;
mod supported;
mod types;

pub use accepts::{accepts_from_env, AcceptsBuildError};
pub use facilitator::{FacilitatorClient, FacilitatorError};
pub use sla_escrow::{
    sla_escrow_accepts_from_env, sla_escrow_kind_extra_from_supported, SlaEscrowAcceptsError,
};
pub use srm::build_srm_json;
pub use supported::exact_kind_extra_from_supported;
pub use types::{
    encode_payment_response, extract_payment_header_value, parse_payment_header, PaymentParseError,
    PaymentRequired, ResourceInfo,
};

use serde_json::Value;
use std::env::VarError;

/// Configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct SellerConfig {
    /// Base URL of this seller API (used in [`PaymentRequired::resource`] `url`).
    pub public_base_url: String,
    /// pr402 facilitator origin, e.g. `https://preview.ipay.sh` (recommended) or `https://preview.agent.pay402.me` (same APIs; no trailing slash).
    pub facilitator_base_url: String,
    /// Resource description for 402 body.
    pub resource_description: String,
    /// Optional MIME hint for the protected resource.
    pub resource_mime_type: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing or invalid environment variable {0}: {1}")]
    Var(&'static str, String),
}

impl SellerConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        fn req(name: &'static str) -> Result<String, ConfigError> {
            std::env::var(name).map_err(|e: VarError| ConfigError::Var(name, e.to_string()))
        }
        Ok(Self {
            public_base_url: trim_slash(req("SELLER_PUBLIC_BASE_URL")?),
            facilitator_base_url: trim_slash(req("FACILITATOR_BASE_URL")?),
            resource_description: req("SELLER_RESOURCE_DESCRIPTION")
                .unwrap_or_else(|_| "Premium seller API route".into()),
            resource_mime_type: std::env::var("SELLER_RESOURCE_MIME")
                .unwrap_or_else(|_| "application/json".into()),
        })
    }

    /// Paid route path (e.g. `/api/premium`) used in the default [`PaymentRequired`] resource URL.
    pub fn paid_path(&self) -> String {
        std::env::var("SELLER_PAID_PATH").unwrap_or_else(|_| "/api/premium".into())
    }

    /// Free route path for demos.
    pub fn free_path(&self) -> String {
        std::env::var("SELLER_FREE_PATH").unwrap_or_else(|_| "/api/free".into())
    }
}

fn trim_slash(mut s: String) -> String {
    while s.ends_with('/') {
        s.pop();
    }
    s
}

/// Build the x402 v2 payment-required document for a protected resource.
pub fn build_payment_required(
    config: &SellerConfig,
    resource_path: &str,
) -> Result<PaymentRequired, AcceptsBuildError> {
    build_payment_required_with_accepts(config, resource_path, accepts_from_env()?)
}

/// Build PaymentRequired with explicit accepts (exact or sla-escrow).
pub fn build_payment_required_with_accepts(
    config: &SellerConfig,
    resource_path: &str,
    accepts: Vec<Value>,
) -> Result<PaymentRequired, AcceptsBuildError> {
    let mut path = resource_path.to_string();
    if !path.starts_with('/') {
        path.insert(0, '/');
    }
    let url = format!("{}{}", config.public_base_url, path);

    Ok(PaymentRequired {
        x402_version: 2,
        error: None,
        resource: ResourceInfo {
            url,
            description: config.resource_description.clone(),
            mime_type: config.resource_mime_type.clone(),
        },
        accepts,
        extensions: serde_json::json!({
            "pr402FacilitatorUrl": config.facilitator_base_url,
        }),
    })
}

/// Env-driven accepts: `X402_SCHEME=sla-escrow` selects sla-escrow rail.
pub fn accepts_for_env() -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let scheme = std::env::var("X402_SCHEME").unwrap_or_else(|_| "exact".into());
    if scheme == "sla-escrow" {
        Ok(sla_escrow_accepts_from_env()?)
    } else {
        Ok(accepts_from_env()?)
    }
}

/// Serialize [`PaymentRequired`] for an HTTP 402 JSON body.
pub fn payment_required_json(pr: &PaymentRequired) -> Result<Value, serde_json::Error> {
    serde_json::to_value(pr)
}
