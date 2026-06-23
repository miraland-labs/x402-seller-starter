# x402-seller-starter

Reference seller starters for [pr402](https://github.com/miralandlabs/pr402) / [x402 v2](https://github.com/coinbase/x402/blob/main/specs/x402-specification-v2.md), in three languages. Each starter gates one HTTP route with `402 Payment Required` and settles via a pr402 facilitator.

```
x402-seller-starter/
├── rust/          Axum         · ~300 LOC  · library + axum example
├── typescript/    Express 5    · ~300 LOC  · Node ≥ 20, native fetch
└── python/        FastAPI      · ~350 LOC  · Python ≥ 3.11, httpx
```

## Modern Seller Integration Flow

To make integration easier, the x402 ecosystem uses a two-part integration strategy:
1. **Onboarding & Discovery (CLI)**: Use the [x402-cli](file:///Users/miracle17/miraland-labs/x402/tools/x402-cli) tool to check your status, activate your payment vault on-chain, register in the directory, and list your API resources.
2. **Runtime Gate (SDK)**: Use the unified `X402SellerSDK` inside your web application to dynamically fetch capabilities, cache enriched 402 templates in memory, and verify+settle payment proofs.

This eliminates local PDA derivation and blockchain SDK dependencies inside your web application!

---

## Pick a starter

| You write… | Start here | Runtime |
|---|---|---|
| Rust | [`rust/`](rust/README.md) | `cargo run --example axum_server` |
| TypeScript / JavaScript | [`typescript/`](typescript/README.md) | `npm start` |
| Python | [`python/`](python/README.md) | `x402-seller-start` |

Each directory is self-contained: clone this repo, step into your language of choice, and follow the subdir README.

---

## What the starters share

- **X402SellerSDK.** All starters implement a parallel `X402SellerSDK` class/struct. It resolves the facilitator's `/capabilities` at boot time, fetches the fully enriched PaymentRequired challenge template via the `/payment-required/enrich` endpoint, and caches it in memory (polling on a 10-minute interval).
- **Zero Heavy Blockchain Dependencies.** The HTTP API servers themselves do not require heavy Solana dependencies (like `@solana/web3.js` or `solders`). All PDA mapping and validation is performed dynamically via the facilitator.
- **Header contract.** The buyer sends the `PAYMENT-SIGNATURE` header; the server replies with a `PAYMENT-RESPONSE` header (base64 of the settle result) on both success and failure paths.

---

## Onboarding Checklist

Before serving your first 402 response, make sure you complete these steps with the CLI:

1. **Check Status**: Run `node tools/x402-cli/dist/index.js status --keypair <your_keypair.json>` to see your current setup.
2. **Activate Vault**: Run `node tools/x402-cli/dist/index.js activate --wallet <address>` to retrieve the transaction needed to initialize your SplitVault on-chain.
3. **Register Merchant**: Run `node tools/x402-cli/dist/index.js register --keypair <your_keypair.json> --url <api_url> --display-name "My API Store"` to register off-chain.
4. **Enroll APIs**: Write an `x402-resources.json` manifest and register it using `node tools/x402-cli/dist/index.js enroll --manifest <manifest.json> --keypair <your_keypair.json>`.

## License

Apache-2.0.
