//! Helpers for reading facilitator **`GET /supported`** JSON (resource providers).

use serde_json::Value;

/// Returns the `extra` object for the **exact** rail on `network` (e.g. `solana:EtWTRABZaYq6iMfeYKouRu166VU2xqa1`),
/// from a parsed `GET /api/v1/facilitator/supported` body.
pub fn exact_kind_extra_from_supported(supported: &Value, network: &str) -> Option<Value> {
    let kinds = supported.get("kinds")?.as_array()?;
    let kind = kinds.iter().find(|k| {
        k.get("scheme").and_then(|v| v.as_str()) == Some("exact")
            && k.get("network").and_then(|v| v.as_str()) == Some(network)
    })?;
    kind.get("extra").cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_extra_for_network() {
        let supported = json!({
            "kinds": [
                {
                    "scheme": "exact",
                    "network": "solana:testnet",
                    "x402Version": 2,
                    "extra": {"programId": "prog", "feePayer": "fp"}
                }
            ]
        });
        let ex = exact_kind_extra_from_supported(&supported, "solana:testnet").unwrap();
        assert_eq!(ex["programId"], "prog");
        assert_eq!(ex["feePayer"], "fp");
    }

    #[test]
    fn wrong_network_returns_none() {
        let supported = json!({"kinds": [{"scheme":"exact","network":"solana:a","extra":{}}]});
        assert!(exact_kind_extra_from_supported(&supported, "solana:b").is_none());
    }
}
