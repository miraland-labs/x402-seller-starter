//! **Find `payTo` for sellers** (`v2:solana:exact` / UniversalSettle vault).
//!
//! ## What a seller actually needs
//! A value for **`accepts[].payTo`** / **`X402_PAY_TO`** that matches what the facilitator verifies.
//! For the **exact** rail on Solana, that is the **vault PDA** for your merchant key.
//!
//! ## How this example finds it (no `build-tx` unless you opt in)
//! 1. **`GET /api/v1/facilitator/supported`** → `programId` (+ `extra`) for `scheme=exact` and your `X402_NETWORK`.
//! 2. **`MERCHANT_WALLET`** (base58) = your seller pubkey.
//! 3. **Derive** `find_program_address(["vault", merchant], programId)` — same layout as UniversalSettle docs.
//!
//! That is how you **find `payTo` without** calling `build-tx`. Signing an onboard transaction is a **separate**
//! step (provisioning / fee tier); set **`SELLER_FETCH_ONBOARD_TX=1`** only if you also want the facilitator
//! to return an unsigned provisioning tx.
//!
//! **Library dependency note:** only this **example** uses dev-dep `solana-pubkey` (+ `curve25519`) for PDA math.
//! Your seller HTTP service stays `serde` + `reqwest` + `thiserror`.
//!
//! ## Run
//! ```bash
//! MERCHANT_WALLET=<your_base58_pubkey> cargo run --example find_payto
//! ```

use serde_json::Value;
use solana_pubkey::Pubkey;
use std::str::FromStr;

#[derive(Debug)]
struct DemoErr(String);

impl std::fmt::Display for DemoErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DemoErr {}

impl From<String> for DemoErr {
    fn from(s: String) -> Self {
        DemoErr(s)
    }
}

fn e(s: impl Into<String>) -> DemoErr {
    DemoErr(s.into())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), DemoErr> {
    let _ = dotenvy::from_path(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env"),
    );

    let base = std::env::var("FACILITATOR_BASE_URL")
        .map_err(|_| e("Set FACILITATOR_BASE_URL in .env (or export it)."))?;
    let base = base.trim_end_matches('/').to_string();

    let wallet = std::env::var("MERCHANT_WALLET")
        .or_else(|_| std::env::var("SELLER_WALLET"))
        .map_err(|_| {
            e("Set MERCHANT_WALLET (or SELLER_WALLET) to your merchant base58 pubkey.")
        })?;

    let network = std::env::var("X402_NETWORK").unwrap_or_else(|_| {
        "solana:EtWTRABZaYq6iMfeYKouRu166VU2xqa1".to_string()
    });

    let client = reqwest::Client::new();

    let supported: Value = client
        .get(format!("{base}/api/v1/facilitator/supported"))
        .send()
        .await
        .map_err(|x| e(x.to_string()))?
        .error_for_status()
        .map_err(|x| e(x.to_string()))?
        .json()
        .await
        .map_err(|x| e(x.to_string()))?;

    let kinds = supported["kinds"]
        .as_array()
        .ok_or_else(|| e("facilitator /supported: missing kinds[]"))?;

    let exact = kinds
        .iter()
        .find(|k| {
            k.get("scheme").and_then(|v| v.as_str()) == Some("exact")
                && k.get("network").and_then(|v| v.as_str()) == Some(network.as_str())
        })
        .ok_or_else(|| {
            e(format!(
                "No supported kind with scheme=exact and network={network:?}. Run:\n  curl -sS {base}/api/v1/facilitator/supported | jq .kinds"
            ))
        })?;

    let program_id_str = exact["extra"]["programId"]
        .as_str()
        .ok_or_else(|| e("supported kind missing extra.programId"))?;

    let merchant_pk = Pubkey::from_str(wallet.trim())
        .map_err(|x| e(format!("MERCHANT_WALLET parse error: {x}")))?;
    let program_id = Pubkey::from_str(program_id_str)
        .map_err(|x| e(format!("programId parse error: {x}")))?;

    let (vault_pda, _bump) =
        Pubkey::find_program_address(&[b"vault", merchant_pk.as_ref()], &program_id);

    println!("\n═══ find_payto — seller needs this for 402 / .env ═══\n");
    println!("Facilitator: {base}");
    println!("MERCHANT_WALLET: {wallet}");
    println!("X402_NETWORK:      {network}");
    println!("programId:         {program_id_str}\n");

    println!("╔════════════════════════════════════════════════════════════════════════╗");
    println!("║  payTo  (vault PDA for v2:solana:exact — NOT your merchant wallet)   ║");
    println!("╠════════════════════════════════════════════════════════════════════════╣");
    println!("║  {vault_pda}");
    println!("╚════════════════════════════════════════════════════════════════════════╝\n");
    println!("X402_PAY_TO={vault_pda}\n");

    if let Some(extra) = exact.get("extra") {
        let compact = serde_json::to_string(extra).map_err(|x| e(x.to_string()))?;
        println!("Exact rail → facilitator `extra` (required for most pr402 verify paths).");
        println!("Paste into .env (single-quoted so inner `\"` are fine):");
        println!("X402_ACCEPTS_EXTRA_JSON='{compact}'\n");
    }

    println!(
        "/supported kind used:\n{}\n",
        serde_json::to_string_pretty(exact).map_err(|x| e(x.to_string()))?
    );

    if std::env::var("SELLER_FETCH_ONBOARD_TX")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        let onboard_url = format!("{base}/api/v1/facilitator/onboard/build-tx?wallet={wallet}");
        println!("(opt-in SELLER_FETCH_ONBOARD_TX) GET {onboard_url}\n");
        let build: Value = client
            .get(&onboard_url)
            .send()
            .await
            .map_err(|x| e(x.to_string()))?
            .error_for_status()
            .map_err(|x| e(x.to_string()))?
            .json()
            .await
            .map_err(|x| e(x.to_string()))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&build).map_err(|x| e(x.to_string()))?
        );
        println!();
    }

    println!("Next: put X402_PAY_TO + X402_ACCEPTS_EXTRA_JSON in .env, then:  cargo run --example axum_server\n");
    println!("Other rails (e.g. sla-escrow) use a different payTo derivation — this example is exact-only.\n");

    Ok(())
}
