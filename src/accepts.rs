//! Build `accepts[]` from `X402_ACCEPTS_JSON` or discrete env vars.

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AcceptsBuildError {
    #[error("X402_ACCEPTS_JSON is invalid JSON array: {0}")]
    AcceptsJson(String),
    #[error(
        "missing or empty environment variable `{0}`. Set `X402_ACCEPTS_JSON` or all of: X402_SCHEME, X402_NETWORK, X402_ASSET, X402_AMOUNT, X402_PAY_TO, X402_MAX_TIMEOUT_SECONDS"
    )]
    MissingDiscreteVar(&'static str),
    #[error("invalid X402_MAX_TIMEOUT_SECONDS (expected a non-negative integer): {0}")]
    InvalidMaxTimeout(String),
    #[error("optional X402_ACCEPTS_EXTRA_JSON invalid: {0}")]
    ExtraJson(String),
}

fn discrete_var(name: &'static str) -> Result<String, AcceptsBuildError> {
    let v = std::env::var(name).map_err(|_| AcceptsBuildError::MissingDiscreteVar(name))?;
    if v.trim().is_empty() {
        return Err(AcceptsBuildError::MissingDiscreteVar(name));
    }
    Ok(v)
}

/// Build `accepts` array for a [`super::PaymentRequired`](crate::PaymentRequired).
pub fn accepts_from_env() -> Result<Vec<Value>, AcceptsBuildError> {
    if let Ok(raw) = std::env::var("X402_ACCEPTS_JSON") {
        let v: Value = serde_json::from_str(raw.trim()).map_err(|e| {
            AcceptsBuildError::AcceptsJson(e.to_string())
        })?;
        let arr = v
            .as_array()
            .ok_or_else(|| AcceptsBuildError::AcceptsJson("expected JSON array".into()))?;
        return Ok(arr.clone());
    }

    let scheme = discrete_var("X402_SCHEME")?;
    let network = discrete_var("X402_NETWORK")?;
    let asset = discrete_var("X402_ASSET")?;
    let amount = discrete_var("X402_AMOUNT")?;
    let pay_to = discrete_var("X402_PAY_TO")?;
    let max_timeout_raw = discrete_var("X402_MAX_TIMEOUT_SECONDS")?;
    let max_timeout: u64 = max_timeout_raw.parse().map_err(|e| {
        AcceptsBuildError::InvalidMaxTimeout(format!("{max_timeout_raw:?}: {e}"))
    })?;

    let extra: Option<Value> = match std::env::var("X402_ACCEPTS_EXTRA_JSON") {
        Ok(raw) if !raw.trim().is_empty() => Some(
            serde_json::from_str(raw.trim())
                .map_err(|e| AcceptsBuildError::ExtraJson(e.to_string()))?,
        ),
        _ => None,
    };

    let mut line = serde_json::json!({
        "scheme": scheme,
        "network": network,
        "asset": asset,
        "amount": amount,
        "payTo": pay_to,
        "maxTimeoutSeconds": max_timeout,
    });
    if let Some(obj) = line.as_object_mut() {
        if let Some(e) = extra {
            obj.insert("extra".to_string(), e);
        }
    }
    Ok(vec![line])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_x402_env() {
        for k in [
            "X402_ACCEPTS_JSON",
            "X402_SCHEME",
            "X402_NETWORK",
            "X402_ASSET",
            "X402_AMOUNT",
            "X402_PAY_TO",
            "X402_MAX_TIMEOUT_SECONDS",
            "X402_ACCEPTS_EXTRA_JSON",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    fn discrete_accepts_merges_extra() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_x402_env();
        std::env::set_var("X402_SCHEME", "v2:solana:exact");
        std::env::set_var("X402_NETWORK", "solana:dev");
        std::env::set_var("X402_ASSET", "So11111111111111111111111111111111111111112");
        std::env::set_var("X402_AMOUNT", "1");
        std::env::set_var("X402_PAY_TO", "vault111111111111111111111111111111111111111");
        std::env::set_var("X402_MAX_TIMEOUT_SECONDS", "60");
        std::env::set_var("X402_ACCEPTS_EXTRA_JSON", r#"{"feePayer":"Fp","programId":"Pg"}"#);
        let v = accepts_from_env().unwrap();
        assert_eq!(v.len(), 1);
        let row = &v[0];
        assert_eq!(row["scheme"], "v2:solana:exact");
        assert_eq!(row["extra"]["feePayer"], "Fp");
        clear_x402_env();
    }

    #[test]
    fn accepts_json_wins_over_discrete() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_x402_env();
        std::env::set_var(
            "X402_ACCEPTS_JSON",
            r#"[{"scheme":"v2:solana:exact","network":"solana:x","asset":"a","amount":"1","payTo":"p","maxTimeoutSeconds":1,"extra":{}}]"#,
        );
        std::env::set_var("X402_SCHEME", "wrong");
        let v = accepts_from_env().unwrap();
        assert_eq!(v[0]["network"], "solana:x");
        clear_x402_env();
    }
}
