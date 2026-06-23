# x402-seller-starter · Rust

Reference Rust seller for [pr402](https://github.com/miralandlabs/pr402) / [x402 v2](https://github.com/coinbase/x402/blob/main/specs/x402-specification-v2.md). It implements the `X402SellerSDK` struct and an Axum handler wrapper that intercepts requests, gates them with HTTP 402, and settles payments via a pr402 facilitator.

## Scope

Targets the **`exact`** (UniversalSettle) rail — instant micro-payments.

## Quick start

Requires Rust toolchain installed.

1. **Configure environment**: Copy `.env.example` to `.env` and fill in:
   - `FACILITATOR_BASE_URL` (e.g. `https://preview.ipay.sh`)
   - `MERCHANT_WALLET` (your public wallet address)
   - `SELLER_PUBLIC_BASE_URL` (your server's public URL, e.g. `http://127.0.0.1:3000`)
   - `X402_AMOUNT` (USDC price per request, in decimals)

2. **Onboard**: Follow the [x402-cli README](https://github.com/miraland-labs/x402/blob/main/tools/x402-cli/README.md) to activate your vault on-chain and register your merchant wallet off-chain.

3. **Run**:
   ```bash
   cargo run --example axum_server
   ```

4. **Verify**:
   ```bash
   curl -sS http://127.0.0.1:3000/api/free
   curl -i  http://127.0.0.1:3000/api/premium                # Returns HTTP 402 with enriched challenge JSON
   ```

---

## Code Usage

You can easily integrate this into your existing Axum application:

```rust
use axum::{
    routing::get,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json, Router
};
use std::sync::Arc;
use serde_json::json;
use x402_seller_starter::{
    X402SellerSDK,
    extract_payment_header_value,
    parse_payment_header,
    payment_required_json,
    encode_payment_response
};

#[derive(Clone)]
struct AppState {
    sdk: X402SellerSDK,
    paid_path: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let sdk = X402SellerSDK::new(
        "https://preview.ipay.sh",
        "your_wallet_address",
        "http://127.0.0.1:3000",
        "50000", // amount in USDC micro-units
        None, None, None, None
    )?;

    // Start capability resolution and background cache refresh loop
    sdk.start().await?;

    let state = Arc::new(AppState {
        sdk,
        paid_path: "/api/premium".to_string(),
    });

    let app = Router::new()
        .route("/api/premium", get(paid_gate).post(paid_gate))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn paid_gate(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    let pr = s.sdk.get_payment_required(&s.paid_path).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Extract PAYMENT-SIGNATURE
    let raw_payment = extract_payment_header_value(|name| {
        headers.get(name).and_then(|v| v.to_str().ok()).map(String::from)
    });

    let Some(raw) = raw_payment else {
        let body = payment_required_json(&pr.clone().with_error("PAYMENT-SIGNATURE header is required"))
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok((StatusCode::PAYMENT_REQUIRED, Json(body)).into_response());
    };

    let proof = match parse_payment_header(&raw) {
        Ok(v) => v,
        Err(e) => {
            let body = payment_required_json(&pr.clone().with_error(format!("Invalid payment header: {e}")))
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            return Ok((StatusCode::PAYMENT_REQUIRED, Json(body)).into_response());
        }
    };

    // Verify and settle via the facilitator
    match s.sdk.verify_and_settle(&proof).await {
        Ok(settled) => {
            let mut res = Json(json!({ "tier": "paid", "settlement": settled })).into_response();
            if let Ok(hv) = axum::http::HeaderValue::from_str(&encode_payment_response(&settled)) {
                res.headers_mut().insert("PAYMENT-RESPONSE", hv);
            }
            Ok(res)
        }
        Err(e) => {
            let error_result = json!({"success": false, "errorReason": e.to_string()});
            let body = payment_required_json(&pr.clone().with_error(format!("Facilitator: {e}")))
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let mut res = (StatusCode::PAYMENT_REQUIRED, Json(body)).into_response();
            if let Ok(hv) = axum::http::HeaderValue::from_str(&encode_payment_response(&error_result)) {
                res.headers_mut().insert("PAYMENT-RESPONSE", hv);
            }
            Ok(res)
        }
    }
}
```

---

## File Layout

| Path | Purpose |
| --- | --- |
| `src/lib.rs` | Main exports, config and serializations. |
| `src/facilitator.rs` | Defines `X402SellerSDK` and `FacilitatorClient`. |
| `examples/axum_server.rs` | Complete Axum example app demonstrating paid gates. |