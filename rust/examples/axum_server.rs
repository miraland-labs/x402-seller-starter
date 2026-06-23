//! Example Axum server: free route, paid route with HTTP 402 + `PAYMENT-SIGNATURE` settlement via pr402.
//!
//! x402 v2 wire flow this example implements:
//!   - Server -> Client: HTTP 402 with the payment-required JSON **in the response body**
//!     (the spec also defines an optional `PAYMENT-REQUIRED` response header; this example
//!     does not emit it since the JSON body is the interop default and what pr402 buyers read).
//!   - Client -> Server: `PAYMENT-SIGNATURE` request header carrying the signed proof JSON.
//!   - Server -> Client: `PAYMENT-RESPONSE` response header (base64 JSON of the settle result),
//!     emitted on both HTTP 200 (success) and HTTP 402 (failed / retry-needed) after the
//!     facilitator verify+settle attempt.
//!
//! Run (after `cp .env.example .env` the example loads `.env` automatically; or export vars yourself).
//! Set **`X402_PAY_TO`** — see README *Quick start → Step 0* (`cargo run --example find_payto`).
//! ```bash
//! export SELLER_PUBLIC_BASE_URL="http://127.0.0.1:3000"
//! export FACILITATOR_BASE_URL="https://your-pr402-deployment.example"
//! # Either paste accepts from facilitator /supported (+ payTo from find_payto):
//! export X402_ACCEPTS_JSON='[{"scheme":"v2:solana:exact",...}]'
//! # …or set X402_SCHEME, X402_NETWORK, X402_ASSET, X402_AMOUNT, X402_PAY_TO, X402_MAX_TIMEOUT_SECONDS
//! cargo run --example axum_server
//! ```

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use x402_seller_starter::{
    build_srm_json, encode_payment_response, extract_payment_header_value,
    parse_payment_header, payment_required_json, X402SellerSDK,
};


#[derive(Clone)]
struct AppState {
    paid_path: String,
    free_path: String,
    sdk: X402SellerSDK,
    public_base_url: String,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let env_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    match dotenvy::from_path(&env_path) {
        Ok(()) => {}
        Err(e) if e.not_found() => {}
        Err(e) => {
            return Err(format!("failed to load {}: {e}", env_path.display()).into());
        }
    }
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let paid_path = std::env::var("SELLER_PAID_PATH").unwrap_or_else(|_| "/api/premium".into());
    let free_path = std::env::var("SELLER_FREE_PATH").unwrap_or_else(|_| "/api/free".into());

    let facilitator_url = std::env::var("FACILITATOR_BASE_URL")
        .unwrap_or_else(|_| "https://preview.ipay.sh".to_string());
    let seller_wallet = std::env::var("MERCHANT_WALLET")
        .or_else(|_| std::env::var("SELLER_WALLET"))
        .unwrap_or_else(|_| "your_wallet_address".to_string());
    let public_base_url = std::env::var("SELLER_PUBLIC_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
    let amount = std::env::var("X402_AMOUNT").unwrap_or_else(|_| "50000".to_string());
    let scheme = std::env::var("X402_SCHEME").ok();
    let asset = std::env::var("X402_ASSET").ok();
    let network = std::env::var("X402_NETWORK").ok();
    let max_timeout_seconds = std::env::var("X402_MAX_TIMEOUT_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());

    let sdk = X402SellerSDK::new(
        &facilitator_url,
        &seller_wallet,
        &public_base_url,
        &amount,
        scheme.as_deref(),
        asset.as_deref(),
        network.as_deref(),
        max_timeout_seconds,
    )?;

    sdk.start().await?;

    let state = Arc::new(AppState {
        paid_path: paid_path.clone(),
        free_path: free_path.clone(),
        sdk,
        public_base_url,
    });

    let app = Router::new()
        .route("/", get(root))
        .route("/.well-known/x402-resources.json", get(srm_json))
        .route(&free_path, get(free_ok))
        .route(&paid_path, get(paid_gate).post(paid_gate))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("listening on http://{bind} free={free_path} paid={paid_path}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn root(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    Json(json!({
        "service": "x402-seller-starter",
        "free": s.free_path,
        "paid": s.paid_path,
        "facilitator": s.sdk.get_payment_required(&s.paid_path).await.ok().and_then(|pr| pr.extensions.get("pr402FacilitatorUrl").and_then(|v| v.as_str().map(String::from))),
        "docs": "https://github.com/miraland-labs/x402",
    }))
}

async fn free_ok() -> impl IntoResponse {
    Json(json!({ "tier": "free", "message": "no payment required" }))
}

async fn srm_json(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    let scheme = std::env::var("X402_SCHEME").unwrap_or_else(|_| "exact".into());
    // Create temporary seller config mock for srm backward compatibility
    let mock_config = x402_seller_starter::SellerConfig {
        public_base_url: s.public_base_url.clone(),
        facilitator_base_url: "".to_string(),
        resource_description: "Paid API".to_string(),
        resource_mime_type: "application/json".to_string(),
    };
    Json(build_srm_json(&mock_config, &s.paid_path, &scheme))
}

async fn paid_gate(
    State(s): State<Arc<AppState>>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or(&s.paid_path);
    let pr = s.sdk.get_payment_required(path_and_query).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // x402 v2: PAYMENT-SIGNATURE only
    let raw_payment = extract_payment_header_value(|name| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
    });

    let Some(raw) = raw_payment else {
        let body =
            payment_required_json(&pr.clone().with_error("PAYMENT-SIGNATURE header is required (x402 v2)"))
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok((StatusCode::PAYMENT_REQUIRED, Json(body)).into_response());
    };

    let proof = match parse_payment_header(&raw) {
        Ok(v) => v,
        Err(e) => {
            let body =
                payment_required_json(&pr.clone().with_error(format!("Invalid payment header: {e}")))
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            return Ok((StatusCode::PAYMENT_REQUIRED, Json(body)).into_response());
        }
    };

    match s.sdk.verify_and_settle(&proof).await {
        Ok(settled) => {
            let message = if settled
                .get("settlementNote")
                .and_then(|v| v.as_str())
                .is_some()
            {
                "payment verified; settlement already on-chain (idempotent)"
            } else {
                "payment verified and settled"
            };
            let body = json!({
                "tier": "paid",
                "message": message,
                "settlement": settled,
            });
            // x402 v2: emit PAYMENT-RESPONSE header with base64-encoded settlement result
            let mut res = Json(&body).into_response();
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
            // x402 v2: emit PAYMENT-RESPONSE on failure too
            if let Ok(hv) =
                axum::http::HeaderValue::from_str(&encode_payment_response(&error_result))
            {
                res.headers_mut().insert("PAYMENT-RESPONSE", hv);
            }
            Ok(res)
        }
    }
}
