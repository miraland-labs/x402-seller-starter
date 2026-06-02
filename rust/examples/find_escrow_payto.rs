//! Find sla-escrow `payTo` (escrow PDA) via facilitator discovery rail.
//!
//! ```bash
//! MERCHANT_WALLET=<pubkey> FACILITATOR_BASE_URL=https://preview.ipay.sh cargo run --example find_escrow_payto
//! ```

use serde_json::Value;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = dotenvy::from_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env"));
    let base = std::env::var("FACILITATOR_BASE_URL")?
        .trim_end_matches('/')
        .to_string();
    let wallet = std::env::var("MERCHANT_WALLET").or_else(|_| std::env::var("SELLER_WALLET"))?;
    let asset = std::env::var("X402_ASSET").unwrap_or_else(|_| "USDC".to_string());
    let url = format!("{base}/api/v1/facilitator/sellers/{wallet}/rails/sla-escrow?asset={asset}");
    let info: Value = reqwest::Client::new()
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let pay_to = info["payTo"]
        .as_str()
        .ok_or("discovery response missing payTo")?;

    println!("\n═══ find_escrow_payto (wire-only sla-escrow starter) ═══\n");
    println!("X402_SCHEME=sla-escrow");
    println!("X402_PAY_TO={pay_to}");
    println!("X402_MERCHANT_WALLET={wallet}");
    println!("\nSet ORACLE_AUTHORITIES, ORACLE_PROFILE_ID, ORACLE_NORMATIVE_SPEC_URL in .env.");
    println!("Delivery / SubmitDelivery are out of scope — fund-only wire path.\n");
    Ok(())
}
