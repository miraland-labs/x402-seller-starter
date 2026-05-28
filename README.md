# x402-seller-starter

Reference seller starters for [pr402](https://github.com/miralandlabs/pr402) / [x402 v2](https://github.com/coinbase/x402/blob/main/specs/x402-specification-v2.md), in three languages. Each starter gates one HTTP route with `402 Payment Required` and settles via a pr402 facilitator — nothing more.

```
x402-seller-starter/
├── rust/          Axum         · ~300 LOC  · library + find_payto example
├── typescript/    Express 5    · ~300 LOC  · Node ≥ 20, native fetch
└── python/        FastAPI      · ~350 LOC  · Python ≥ 3.11, httpx + solders
```

All three ship the same feature set, use the same env var names, produce the same 402 body shape on the wire, and target the **`exact`** (UniversalSettle) rail.

## Who this is for

- You are **already writing a paid API** in Rust, TypeScript, or Python and want a concrete reference to copy the 402 middleware pattern from.
- You want to understand **exactly what an x402 v2 seller does on the wire**, free of framework opinions.

## Who this is *not* for

- You want to spin up a paid API from scratch in a different language (Go, Java, Elixir, …). The README for each starter is dense enough that porting the ideas takes under an hour — the `accepts_from_env` helper and the two-POST `verify_and_settle` loop is the whole contract.
- You are a seller looking for a hosted onboarding UI — go to [ipay.sh](https://ipay.sh) instead. It walks through Preview → Activate → Verify in about 90 seconds, no code required.

## Pick a starter

| You write… | Start here | Runtime |
|---|---|---|
| Rust | [`rust/`](rust/README.md) | `cargo run --example axum_server` |
| TypeScript / JavaScript | [`typescript/`](typescript/README.md) | `npm start` (needs Node ≥ 20) |
| Python | [`python/`](python/README.md) | `x402-seller-start` (needs Python ≥ 3.11) |

Each directory is self-contained: clone this repo, step into your language of choice, and follow the subdir README. There is no root-level build.

## What the starters share

- **Scope.** All three are limited to `v2:solana:exact`. For `sla-escrow`, see the [pr402 SLA docs](https://ipay.sh/onboarding_guide.md).
- **Env var names.** `SELLER_PUBLIC_BASE_URL`, `FACILITATOR_BASE_URL`, `MERCHANT_WALLET`, `X402_SCHEME`, `X402_NETWORK`, `X402_ASSET`, `X402_AMOUNT`, `X402_PAY_TO`, `X402_MAX_TIMEOUT_SECONDS`, `X402_ACCEPTS_EXTRA_JSON` — identical across languages.
- **`find_payto` companion.** Each starter includes a small script that computes your vault PDA (`payTo`) from `/supported` + `MERCHANT_WALLET`, then prints a ready-to-paste `X402_ACCEPTS_EXTRA_JSON=...` line.
- **Header contract.** Buyer sends `PAYMENT-SIGNATURE` (raw JSON or base64-of-JSON); server replies with a `PAYMENT-RESPONSE` header (base64 of the settle result) on both success and failure paths.
- **The 402 body.** Every starter emits the same `extensions.pr402FacilitatorUrl` hint so buyers can skip a `/supported` probe when they recognize the key.

## Before serving your first 402

Whichever starter you pick:

1. Run the `find_payto` script and paste its two lines into your `.env`.
2. **Activate your vault on-chain** at [ipay.sh](https://ipay.sh) (or via `POST /api/v1/facilitator/sellers/provision-tx`). Without this, your 402 responses are valid but settle will return `409 Conflict — vault not yet on-chain`.

## License

Apache-2.0.
