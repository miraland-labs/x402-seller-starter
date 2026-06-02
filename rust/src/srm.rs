//! Minimal Seller Resource Manifest (SRM) stub for discovery harvest.

use crate::SellerConfig;
use serde_json::{json, Value};

pub fn build_srm_json(config: &SellerConfig, resource_path: &str, scheme: &str) -> Value {
    let mut path = resource_path.to_string();
    if !path.starts_with('/') {
        path.insert(0, '/');
    }
    let resource_url = format!("{}{}", config.public_base_url, path);
    let merchant = std::env::var("X402_MERCHANT_WALLET")
        .or_else(|_| std::env::var("MERCHANT_WALLET"))
        .or_else(|_| std::env::var("SELLER_WALLET"))
        .unwrap_or_default();
    let slug = path.trim_start_matches('/').replace('/', "-");
    json!({
        "schemaVersion": "0.1.0",
        "origin": config.public_base_url,
        "merchantWallet": merchant,
        "facilitatorHint": config.facilitator_base_url,
        "resources": [{
            "id": slug,
            "title": config.resource_description,
            "method": "GET",
            "resourceUrl": resource_url,
            "scheme": scheme,
            "tags": ["starter"]
        }]
    })
}
