//! sla-escrow accepts[] builder (wire-only — FundPayment verify/settle, no delivery path).

use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SlaEscrowAcceptsError {
    #[error("missing or empty `{0}`")]
    MissingVar(&'static str),
    #[error("invalid X402_MAX_TIMEOUT_SECONDS: {0}")]
    InvalidTimeout(String),
    #[error("invalid X402_ACCEPTS_EXTRA_JSON: {0}")]
    ExtraJson(String),
}

fn req(name: &'static str) -> Result<String, SlaEscrowAcceptsError> {
    let v = std::env::var(name).map_err(|_| SlaEscrowAcceptsError::MissingVar(name))?;
    if v.trim().is_empty() {
        return Err(SlaEscrowAcceptsError::MissingVar(name));
    }
    Ok(v)
}

/// Build sla-escrow `accepts[]` from env (wire-only starter).
pub fn sla_escrow_accepts_from_env() -> Result<Vec<Value>, SlaEscrowAcceptsError> {
    if let Ok(raw) = std::env::var("X402_ACCEPTS_JSON") {
        let v: Value = serde_json::from_str(raw.trim())
            .map_err(|e| SlaEscrowAcceptsError::ExtraJson(e.to_string()))?;
        let arr = v
            .as_array()
            .ok_or_else(|| SlaEscrowAcceptsError::ExtraJson("expected array".into()))?;
        return Ok(arr.clone());
    }

    let network = req("X402_NETWORK")?;
    let asset = req("X402_ASSET")?;
    let amount = req("X402_AMOUNT")?;
    let pay_to = req("X402_PAY_TO")?;
    let max_timeout_raw = req("X402_MAX_TIMEOUT_SECONDS")?;
    let max_timeout: u64 = max_timeout_raw
        .parse()
        .map_err(|e| SlaEscrowAcceptsError::InvalidTimeout(format!("{max_timeout_raw}: {e}")))?;

    let merchant = std::env::var("X402_MERCHANT_WALLET")
        .or_else(|_| std::env::var("MERCHANT_WALLET"))
        .or_else(|_| std::env::var("SELLER_WALLET"))
        .unwrap_or_default();

    let oracle_authorities: Vec<String> = std::env::var("ORACLE_AUTHORITIES")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    let profile_id =
        std::env::var("ORACLE_PROFILE_ID").unwrap_or_else(|_| "x402/oracles/api-quality/v1".into());
    let normative = std::env::var("ORACLE_NORMATIVE_SPEC_URL").unwrap_or_default();

    let mut extra = json!({
        "merchantWallet": merchant,
        "oracleAuthorities": oracle_authorities,
        "oracleProfiles": [{
            "profileId": profile_id,
            "normativeSpecUrl": normative,
        }],
    });

    if let Ok(raw) = std::env::var("X402_ACCEPTS_EXTRA_JSON") {
        if !raw.trim().is_empty() {
            let patch: Value = serde_json::from_str(raw.trim())
                .map_err(|e| SlaEscrowAcceptsError::ExtraJson(e.to_string()))?;
            if let (Some(base), Some(over)) = (extra.as_object_mut(), patch.as_object()) {
                for (k, v) in over {
                    base.insert(k.clone(), v.clone());
                }
            }
        }
    }

    Ok(vec![json!({
        "scheme": "sla-escrow",
        "network": network,
        "asset": asset,
        "amount": amount,
        "payTo": pay_to,
        "maxTimeoutSeconds": max_timeout,
        "extra": extra,
    })])
}

/// Merge sla-escrow rail `extra` from facilitator GET /supported.
pub fn sla_escrow_kind_extra_from_supported(supported: &Value, network: &str) -> Option<Value> {
    let kinds = supported.get("kinds")?.as_array()?;
    let kind = kinds.iter().find(|k| {
        k.get("scheme").and_then(|v| v.as_str()) == Some("sla-escrow")
            && k.get("network").and_then(|v| v.as_str()) == Some(network)
    })?;
    kind.get("extra").cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_profiles_invariant() {
        std::env::remove_var("X402_ACCEPTS_JSON");
        std::env::set_var("X402_NETWORK", "solana:EtWTRABZaYq6iMfeYKouRu166VU2xqa1");
        std::env::set_var("X402_ASSET", "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        std::env::set_var("X402_AMOUNT", "1000");
        std::env::set_var("X402_PAY_TO", "EscrowPda1111111111111111111111111111111111");
        std::env::set_var("X402_MAX_TIMEOUT_SECONDS", "300");
        std::env::set_var(
            "ORACLE_AUTHORITIES",
            "OracleAuth1111111111111111111111111111111111",
        );
        let accepts = sla_escrow_accepts_from_env().unwrap();
        let profiles = accepts[0]["extra"]["oracleProfiles"].as_array().unwrap();
        assert_eq!(profiles.len(), 1);
        assert!(profiles[0]["profileId"].is_string());
    }
}
