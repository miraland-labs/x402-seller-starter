//! Minimal pr402 verify + settle client.

use reqwest::Url;
use serde_json::{json, Value};
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
    /// If **settle** fails with an on-chain “already processed” error but **verify** succeeded,
    /// returns a synthetic success value so the seller can still return paid content (idempotent /
    /// older facilitators). Current pr402 may already normalize duplicate settle to HTTP 200.
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

        let settle_res = self
            .client
            .post(self.settle_url.clone())
            .header("Content-Type", "application/json")
            .json(body)
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
        "transaction": null,
        "settlementNote": "verify succeeded; settle reported duplicate on-chain — treating as idempotent success",
        "settleErrorPreview": settle_error_snippet.chars().take(240).collect::<String>(),
    })
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
    }
}
