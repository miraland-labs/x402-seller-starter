//! Minimal pr402 verify + settle client.

use reqwest::Url;
use serde_json::{json, Value};
#[cfg(feature = "sdk")]
use std::sync::Arc;
use thiserror::Error;


#[derive(Debug, Error)]
pub enum FacilitatorError {
    #[error("invalid facilitator base URL: {0}")]
    Url(String),
    #[error("HTTP {status}: {body}")]
    Http {
        status: u16,
        body: String,
        step: &'static str,
    },
    #[error("request failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("settle response is not valid JSON: {0}")]
    InvalidSettleJson(String),
}

#[derive(Clone)]
pub struct FacilitatorClient {
    verify_url: Url,
    settle_url: Url,
    client: reqwest::Client,
}

impl FacilitatorClient {
    pub fn new(facilitator_base: &str) -> Result<Self, FacilitatorError> {
        let base = facilitator_base.trim_end_matches('/');
        let verify = Url::parse(&format!("{}/api/v1/facilitator/verify", base))
            .map_err(|e| FacilitatorError::Url(e.to_string()))?;
        let settle = Url::parse(&format!("{}/api/v1/facilitator/settle", base))
            .map_err(|e| FacilitatorError::Url(e.to_string()))?;
        Ok(Self {
            verify_url: verify,
            settle_url: settle,
            client: reqwest::Client::new(),
        })
    }

    /// POST the same JSON body to verify then settle; returns the settle response JSON.
    ///
    /// **Legacy fallback for duplicate settle.** Current pr402 normalizes duplicate on-chain
    /// settle attempts (e.g. "This transaction has already been processed") into HTTP 200 with
    /// an idempotent success response, so you will rarely see the fallback branch fire. We
    /// keep it so this starter also runs against older pr402 versions and any facilitator
    /// that hasn't normalized duplicate-settle yet: if settle returns HTTP 4xx/5xx with a
    /// duplicate-processed signature in the body and verify had succeeded, we synthesize a
    /// success value so the seller can still return paid content (payment already landed).
    ///
    /// **`isValid: false` is not an HTTP error.** The facilitator returns HTTP 200 with
    /// `{ isValid: false, invalidReason: "..." }` for semantically invalid proofs (wrong
    /// amount, wrong payee, expired blockhash, etc.). Those are caught by
    /// `verify_json_indicates_valid` and converted to `FacilitatorError::Http` so the seller
    /// can surface `invalidReason` to the buyer. Sellers that want to forward the reason
    /// verbatim should match on the `body` field of the returned error.
    pub async fn verify_and_settle(&self, body: &Value) -> Result<Value, FacilitatorError> {
        let verify_res = self
            .client
            .post(self.verify_url.clone())
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await?;
        let status = verify_res.status();
        let verify_text = verify_res.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(FacilitatorError::Http {
                status: status.as_u16(),
                body: verify_text,
                step: "verify",
            });
        }

        let verify_value: Value = serde_json::from_str(&verify_text).map_err(|e| {
            FacilitatorError::InvalidSettleJson(format!(
                "verify response not JSON: {e}; body_prefix={}",
                verify_text.chars().take(300).collect::<String>()
            ))
        })?;

        if !verify_json_indicates_valid(&verify_value) {
            return Err(FacilitatorError::Http {
                status: status.as_u16(),
                body: verify_text,
                step: "verify",
            });
        }

        let mut settle_body = body.clone();
        if let Some(cid) = verify_value
            .get("correlationId")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            merge_correlation_id(&mut settle_body, cid);
        }

        let settle_res = self
            .client
            .post(self.settle_url.clone())
            .header("Content-Type", "application/json")
            .json(&settle_body)
            .send()
            .await?;
        let status = settle_res.status();
        let settle_text = settle_res.text().await.unwrap_or_default();
        if !status.is_success() {
            if is_duplicate_settle_body(&settle_text) {
                return Ok(synthetic_settlement_after_duplicate(
                    &verify_value,
                    body,
                    &settle_text,
                ));
            }
            return Err(FacilitatorError::Http {
                status: status.as_u16(),
                body: settle_text,
                step: "settle",
            });
        }
        serde_json::from_str(&settle_text).map_err(|e| {
            FacilitatorError::InvalidSettleJson(format!(
                "{e}; status={}; body_prefix={}",
                status.as_u16(),
                settle_text.chars().take(500).collect::<String>()
            ))
        })
    }
}

#[cfg(feature = "sdk")]
use std::time::Duration;
#[cfg(feature = "sdk")]
use tokio::sync::RwLock;
#[cfg(feature = "sdk")]
use crate::types::PaymentRequired;

#[cfg(feature = "sdk")]
#[derive(Clone)]
pub struct X402SellerSDK {
    inner: Arc<X402SellerSDKInner>,
}

#[cfg(feature = "sdk")]
struct X402SellerSDKInner {
    facilitator_url: String,
    seller_wallet: String,
    public_base_url: String,
    amount: String,
    scheme: String,
    asset: Option<String>,
    network: Option<String>,
    max_timeout_seconds: u64,
    cached_body: RwLock<Option<PaymentRequired>>,
    facilitator_client: FacilitatorClient,
    http_client: reqwest::Client,
}

#[cfg(feature = "sdk")]
impl X402SellerSDK {
    pub fn new(
        facilitator_url: &str,
        seller_wallet: &str,
        public_base_url: &str,
        amount: &str,
        scheme: Option<&str>,
        asset: Option<&str>,
        network: Option<&str>,
        max_timeout_seconds: Option<u64>,
    ) -> Result<Self, FacilitatorError> {
        let facilitator_client = FacilitatorClient::new(facilitator_url)?;
        let http_client = reqwest::Client::new();
        Ok(Self {
            inner: Arc::new(X402SellerSDKInner {
                facilitator_url: facilitator_url.trim_end_matches('/').to_string(),
                seller_wallet: seller_wallet.to_string(),
                public_base_url: public_base_url.trim_end_matches('/').to_string(),
                amount: amount.to_string(),
                scheme: scheme.unwrap_or("exact").to_string(),
                asset: asset.map(String::from),
                network: network.map(String::from),
                max_timeout_seconds: max_timeout_seconds.unwrap_or(300),
                cached_body: RwLock::new(None),
                facilitator_client,
                http_client,
            }),
        })
    }

    pub async fn start(&self) -> Result<(), FacilitatorError> {
        // Initial fetch
        self.refresh_cache().await?;

        // Background task to refresh the cache every 10 minutes
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(600)).await;
                if let Err(e) = this.refresh_cache().await {
                    eprintln!("[X402SellerSDK] Background cache refresh failed: {:?}", e);
                }
            }
        });

        Ok(())
    }

    async fn refresh_cache(&self) -> Result<(), FacilitatorError> {
        let client = &self.inner.http_client;
        let mut resolved_network = self.inner.network.clone();
        let mut resolved_asset = self.inner.asset.clone();

        // 1. Resolve network and asset via capabilities if missing
        if resolved_network.is_none() || resolved_asset.is_none() {
            let caps_url = format!("{}/api/v1/facilitator/capabilities", self.inner.facilitator_url);
            let caps_res = client
                .get(&caps_url)
                .header("Accept", "application/json")
                .send()
                .await?;
            if !caps_res.status().is_success() {
                return Err(FacilitatorError::Http {
                    status: caps_res.status().as_u16(),
                    body: caps_res.text().await.unwrap_or_default(),
                    step: "capabilities",
                });
            }
            let caps: Value = caps_res.json().await?;
            if resolved_network.is_none() {
                resolved_network = caps.get("solanaNetwork")
                    .or_else(|| caps.get("network"))
                    .and_then(|v| v.as_str())
                    .map(String::from);
            }
            if resolved_asset.is_none() {
                resolved_asset = caps.get("usdcMint")
                    .and_then(|v| v.as_str())
                    .map(String::from);
            }
        }

        let network = resolved_network.unwrap_or_else(|| "solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp".to_string());
        let asset = resolved_asset.unwrap_or_else(|| "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".to_string());

        // 2. Build draft PaymentRequired
        let draft = json!({
            "x402Version": 2,
            "resource": { "url": format!("{}/api/placeholder", self.inner.public_base_url) },
            "accepts": [
                {
                    "scheme": self.inner.scheme,
                    "network": network,
                    "payTo": self.inner.seller_wallet,
                    "asset": asset,
                    "amount": self.inner.amount,
                    "maxTimeoutSeconds": self.inner.max_timeout_seconds,
                }
            ]
        });

        // 3. Post to /enrich
        let enrich_url = format!("{}/api/v1/facilitator/payment-required/enrich", self.inner.facilitator_url);
        let enrich_res = client
            .post(&enrich_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&draft)
            .send()
            .await?;
        
        let status = enrich_res.status();
        let enrich_text = enrich_res.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(FacilitatorError::Http {
                status: status.as_u16(),
                body: enrich_text,
                step: "enrich",
            });
        }

        let enriched: PaymentRequired = serde_json::from_str(&enrich_text).map_err(|e| {
            FacilitatorError::InvalidSettleJson(format!("enrich response not JSON: {e}"))
        })?;

        // 4. Update cache
        let mut lock = self.inner.cached_body.write().await;
        *lock = Some(enriched);

        Ok(())
    }

    pub async fn get_payment_required(&self, resource_path: &str) -> Result<PaymentRequired, FacilitatorError> {
        let lock = self.inner.cached_body.read().await;
        let cached = lock.as_ref().ok_or_else(|| {
            FacilitatorError::Url("SDK not initialized; cached PaymentRequired is None".to_string())
        })?;

        let mut path = resource_path.to_string();
        if !path.starts_with('/') {
            path.insert(0, '/');
        }

        let mut pr = cached.clone();
        pr.resource.url = format!("{}{}", self.inner.public_base_url, path);
        Ok(pr)
    }

    pub async fn verify_and_settle(&self, body: &Value) -> Result<Value, FacilitatorError> {
        self.inner.facilitator_client.verify_and_settle(body).await
    }
}


fn verify_json_indicates_valid(v: &Value) -> bool {
    v.get("isValid").and_then(|x| x.as_bool()) == Some(true)
        || v.get("valid").and_then(|x| x.as_bool()) == Some(true)
}

fn is_duplicate_settle_body(body: &str) -> bool {
    let lower = body.to_lowercase();
    lower.contains("already been processed")
        || lower.contains("alreadyprocessed")
        || lower.contains("this transaction has already been processed")
}

fn network_from_proof(proof: &Value) -> String {
    proof
        .pointer("/paymentRequirements/network")
        .or_else(|| proof.pointer("/payment_requirements/network"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn synthetic_settlement_after_duplicate(
    verify: &Value,
    proof: &Value,
    settle_error_snippet: &str,
) -> Value {
    let payer = verify.get("payer").cloned().unwrap_or(Value::Null);
    let network = network_from_proof(proof);
    json!({
        "success": true,
        "payer": payer,
        "network": network,
        "transaction": "",
        "settlementNote": "verify succeeded; settle reported duplicate on-chain — treating as idempotent success",
        "settleErrorPreview": settle_error_snippet.chars().take(240).collect::<String>(),
    })
}

fn merge_correlation_id(body: &mut Value, cid: &str) {
    if let Some(obj) = body.as_object_mut() {
        if !obj.contains_key("correlationId") {
            obj.insert("correlationId".to_string(), Value::String(cid.to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_settle_detection() {
        assert!(is_duplicate_settle_body(
            "TransactionError: This transaction has already been processed"
        ));
        assert!(is_duplicate_settle_body("AlreadyProcessed"));
        assert!(!is_duplicate_settle_body("insufficient funds"));
    }

    #[test]
    fn verify_valid_detection() {
        assert!(verify_json_indicates_valid(
            &json!({"isValid": true, "payer": "x"})
        ));
        assert!(verify_json_indicates_valid(
            &json!({"valid": true, "payer": "x"})
        ));
        assert!(!verify_json_indicates_valid(&json!({"isValid": false})));
    }

    #[test]
    fn synthetic_settlement_shape() {
        let v = synthetic_settlement_after_duplicate(
            &json!({"payer": "PAYER1", "isValid": true}),
            &json!({"paymentRequirements": {"network": "solana:devnet"}}),
            "already processed",
        );
        assert_eq!(v["success"], true);
        assert_eq!(v["payer"], "PAYER1");
        assert_eq!(v["network"], "solana:devnet");
        assert_eq!(v["transaction"], "");
    }
}
