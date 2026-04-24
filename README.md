# x402-seller-starter

Minimal **Apache-2.0** Rust library plus an **Axum** example for resource providers who want to gate HTTP routes with [x402 v2](https://github.com/coinbase/x402/blob/main/specs/x402-specification-v2.md) and settle through a [pr402](https://github.com/miralandlabs/pr402) facilitator.

This is a teaching/reference crate—no fees, no balance product logic—suitable to link from the [x402 hub](https://github.com/miraland-labs/x402).

## Resource provider checklist (`v2:solana:exact`)

Use this to confirm the starter is **complete** for your own Payment Required service (not just “hello 402”):

1. **`FACILITATOR_BASE_URL`** — same deployment you call for `verify` / `settle`.
2. **`GET /supported`** — pick `scheme: exact` + your **`X402_NETWORK`**; note **`programId`** and full **`extra`**.
3. **`MERCHANT_WALLET`** — your seller pubkey; run **`cargo run --example find_payto`** → copy **`X402_PAY_TO`** and **`X402_ACCEPTS_EXTRA_JSON`** lines (or call **`exact_kind_extra_from_supported`** from this crate on cached `/supported` JSON).
4. **Mint / amount** — **`X402_ASSET`** and **`X402_AMOUNT`** match what buyers will pay (USDC decimals, etc.).
5. **`SELLER_PUBLIC_BASE_URL`** — real origin of **`resource.url`** (tunnel/public host in prod).
6. **Vault on-chain** — correct PDA in JSON does not guarantee the account is initialized; provision per facilitator guide if settlement fails.
7. **Protected route** — example supports **GET and POST** on the paid path; same 402 + `PAYMENT-SIGNATURE` pattern.

## Buyer agent checklist (who sends `PAYMENT-SIGNATURE`)

Use this order so the **seller** is the one that calls facilitator **verify** + **settle** (standard x402 flow):

1. **Request the resource** — `GET`/`POST` paid URL → receive **HTTP 402** and `accepts[]`.
2. **Build unsigned payment** — `POST` facilitator `build-exact-payment-tx` (or your rail’s builder). Use **`scheme`** as required by that endpoint (pr402 accepts both `exact` and `v2:solana:exact` on current deployments).
3. **Sign locally** — buyer signs the transaction at **`payerSignatureIndex`** from the build response.
4. **Send proof to the seller** — retry the same HTTP request with header **`PAYMENT-SIGNATURE:`** set to the JSON body you would send to facilitator **verify** (signed tx inside `paymentPayload`). Legacy `X-PAYMENT` is still accepted for backward compatibility.
5. **Do not double-settle** — do **not** call facilitator **settle** yourself and then send `PAYMENT-SIGNATURE`, unless your architecture intentionally splits roles; the Axum example always **verify+settle**s on the server.

If signing is slow, **rebuild** the unsigned tx so the blockhash stays fresh (see facilitator docs / `retry build` errors).

## Seller Q&A (straight answers)

| Question | Answer |
|----------|--------|
| **What must I have to sell with pr402?** | A correct **`payTo`** in your 402 `accepts[]` (vault for **exact**, escrow PDA for **sla-escrow**, …). Without it, buyers cannot pay and verify cannot match. |
| **Mint allowlist** | If the facilitator sets **`PR402_ALLOWED_PAYMENT_MINTS`**, every **`accepts[].asset`** you publish must be in that list (include **`11111111111111111111111111111111`** for native SOL if applicable). Buyers otherwise fail **`build-*`**, **`/verify`**, and **`/settle`**. |
| **Machine-readable `payTo` rules** | On the facilitator: **`GET /api/v1/facilitator/capabilities`** → **`agentManifest.payToSemantics`** (typically **`/agent-payTo-semantics.json`**). See upstream [agent-integration.md](https://github.com/miralandlabs/pr402/blob/main/public/agent-integration.md). |
| **How do I *find* `payTo` for `v2:solana:exact`?** | **`GET /supported`** gives `programId` for your network. **`payTo`** is the **vault PDA** for your merchant pubkey (deterministic seeds). Run **`cargo run --example find_payto`** — it only needs `/supported` + `MERCHANT_WALLET`; it does **not** call `build-tx` unless you set `SELLER_FETCH_ONBOARD_TX=1`. |
| **What is “onboard” then?** | **Provisioning / incentives** (sign a tx so the vault exists on-chain, fee tier, etc.). That is **not** the same as *looking up* `payTo`: the payout address for exact is still that **vault PDA**, whether or not you have signed onboarding yet. |
| **Do I need `solana-sdk` in my seller app?** | **No.** The **library** is `serde` + `reqwest` + `thiserror`. Only the **`find_payto`** example adds dev-dep **`solana-pubkey`** to derive the PDA. |
| **Do I set `payTo` in `.env`?** | **Yes** for this demo: copy the **`X402_PAY_TO=...`** line from `find_payto`. |

Human-written guide for incentives / JIT paths: [onboarding_guide.md](https://preview.agent.pay402.me/onboarding_guide.md) (same host as your facilitator). Buyer/seller overview: [agent-integration.md](https://github.com/miralandlabs/pr402/blob/main/public/agent-integration.md).

## Layout

| Path                       | Purpose                                                   |
| -------------------------- | --------------------------------------------------------- |
| `src/lib.rs`               | `SellerConfig`, `build_payment_required`, JSON helpers    |
| `src/supported.rs`         | `exact_kind_extra_from_supported` (parse `/supported`)      |
| `src/accepts.rs`           | `accepts[]` from `X402_ACCEPTS_JSON` or discrete env vars |
| `src/facilitator.rs`       | `FacilitatorClient::verify_and_settle`                    |
| `examples/find_payto.rs`   | **Find `payTo`** (exact rail): `/supported` + vault PDA  |
| `examples/axum_server.rs`  | Single Axum app: free route + paid route                  |
| `examples/buyer_pay.md`    | Same buyer steps as above in a copy-paste-oriented page  |

## Quick start (example server)

### Step 0 — Find `payTo` (what sellers actually need)

```bash
cp .env.example .env
# Edit .env: FACILITATOR_BASE_URL, X402_NETWORK, MERCHANT_WALLET, …
cargo run --example find_payto
```

Prints **`X402_PAY_TO=...`**, **`X402_ACCEPTS_EXTRA_JSON='...'`** (from live `/supported`), and the full kind. Optional: **`SELLER_FETCH_ONBOARD_TX=1`** for an unsigned provisioning tx — **not required** for `payTo` / `extra`.

### Run the Axum example

1. Copy `.env.example` → `.env` if needed. Ensure **`X402_PAY_TO`** and **`X402_ACCEPTS_EXTRA_JSON`** match **Step 0** / facilitator (`.env.example` ships preview devnet `extra` — refresh for your deployment). In `.env`, **quote values that contain spaces** (e.g. `SELLER_RESOURCE_DESCRIPTION="..."`) — the loader follows dotenv rules where the first space ends an unquoted value.
2. Set **`SELLER_PUBLIC_BASE_URL`** to the base URL buyers and the facilitator will use for this API (no trailing slash). For local dev, use `http://127.0.0.1:3000` (same host/port as `BIND_ADDR` unless you use a tunnel).
3. Run (the Axum example loads `.env` from the crate root; or `set -a && source .env && set +a` then `cargo run`):

```bash
cargo run --example axum_server
```

4. Try:

```bash
curl -sS "http://127.0.0.1:3000/api/free"
curl -i  "http://127.0.0.1:3000/api/premium"   # 402 + JSON body
```

5. With a valid `PAYMENT-SIGNATURE` proof (JSON string from your buyer flow—the same body you send to pr402 `verify`/`settle`):

```bash
curl -sS "http://127.0.0.1:3000/api/premium" \
  -H "PAYMENT-SIGNATURE: {...}"
```

## Where does payTo come from?

**`GET /api/v1/facilitator/supported`** lists each rail’s **`scheme`**, **`network`**, and **`extra`** (including **`programId`** for exact). For **your** resource, **`payTo`** is still **per seller** because it is derived from **your merchant key** (and rail-specific rules):

| Rail | What `payTo` is | How you learn it (this repo) |
|------|-----------------|------------------------------|
| **`v2:solana:exact`** | **Vault PDA** for your merchant | **`find_payto`**: `/supported` → `programId` + `MERCHANT_WALLET` → print **`X402_PAY_TO`**. Same address the protocol uses as the payout vault; onboarding is about **creating** that account on-chain, not about discovering the address. |
| **SLA escrow** | **Escrow PDA** | Different derivation (mint, bank, …). **`accepts[].extra`** must include **`beneficiary`** or **`merchantWallet`** (seller payout identity) so pr402 build/verify can set **`FundPayment.seller`**. Optional sponsored Solana fees for buyers: **`facilitatorPaysTransactionFees: true`** on `POST .../build-sla-escrow-payment-tx` is allowed only if the **facilitator operator** enables **`PR402_SLA_ESCROW_ALLOW_FACILITATOR_FEE_SPONSORSHIP`** on the **facilitator** host (Vercel/env for pr402)—**not** an env var on your seller service. Use facilitator docs / `build-sla-escrow-payment-tx` for that rail — `find_payto` is **exact-only** today. |

**Practical discovery**

1. Pick `FACILITATOR_BASE_URL` and **`X402_NETWORK`** to match `/supported`.
2. Run **`cargo run --example find_payto`** (or replicate its PDA math in your language).
3. Merge **`extra`** from the matching **`kinds[]`** entry into **`accepts`** (`X402_ACCEPTS_EXTRA_JSON` or full JSON).
4. Optional: read **`capabilities`** for human guides:

   ```bash
   curl -sS "$FACILITATOR_BASE_URL/api/v1/facilitator/capabilities" | jq '.agentManifest'
   ```

```bash
curl -sS "$FACILITATOR_BASE_URL/api/v1/facilitator/supported" | jq '.kinds'
```

## Integrating as a library

```rust
use x402_seller_starter::{SellerConfig, build_payment_required, payment_required_json};

let config = SellerConfig::from_env()?;
let pr = build_payment_required(&config, "/api/your-route")?;
let json = payment_required_json(&pr)?;
// Return HTTP 402 with body = json
```

**`extra` from `/supported` (exact rail):**

```rust
use serde_json::Value;
use x402_seller_starter::exact_kind_extra_from_supported;

fn patch_extra(supported: &Value, network: &str) -> Option<Value> {
    exact_kind_extra_from_supported(supported, network)
}
```

## Operational notes

- **`payTo`** must match what pr402 verifies (vault for exact, escrow PDA for sla-escrow, …). See [Where does payTo come from?](#where-does-payto-come-from).
- **Blockhash / proofs** expire; buyers may need to rebuild txs. Your 402 can stay stable while proofs refresh.
- Production hardening (rate limits, auth, idempotency) is intentionally out of scope here—add in your service.

## License

Apache-2.0