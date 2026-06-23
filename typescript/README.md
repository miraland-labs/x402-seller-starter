# x402-seller-starter · TypeScript

Reference TypeScript seller for [pr402](https://github.com/miralandlabs/pr402) / [x402 v2](https://github.com/coinbase/x402/blob/main/specs/x402-specification-v2.md). It implements the `X402SellerSDK` class and an Express middleware that intercepts requests, gates them with HTTP 402, and settles payments via a pr402 facilitator.

## Scope

Targets the **`exact`** (UniversalSettle) rail — instant micro-payments.

## Quick start

Requires Node.js ≥ 20.

1. **Configure environment**: Copy `.env.example` to `.env` and fill in:
   - `FACILITATOR_BASE_URL` (e.g. `https://preview.ipay.sh`)
   - `MERCHANT_WALLET` (your public wallet address)
   - `SELLER_PUBLIC_BASE_URL` (your server's public URL, e.g. `http://127.0.0.1:3000`)
   - `X402_AMOUNT` (USDC price per request, in decimals)

2. **Onboard**: Follow the [x402-cli README](https://github.com/miraland-labs/x402/blob/main/tools/x402-cli/README.md) to activate your vault on-chain and register your merchant wallet off-chain.

3. **Install & Run**:
   ```bash
   npm install
   npm start
   ```

4. **Verify**:
   ```bash
   curl -sS http://127.0.0.1:3000/api/free
   curl -i  http://127.0.0.1:3000/api/premium                # Returns HTTP 402 with enriched challenge JSON
   ```

---

## Code Usage

You can easily drop this into your existing Express application:

```typescript
import express from "express";
import { X402SellerSDK, x402Middleware } from "./payment-required.js";

const sdk = new X402SellerSDK({
  facilitatorUrl: process.env.FACILITATOR_BASE_URL!,
  sellerWallet: process.env.MERCHANT_WALLET!,
  publicBaseUrl: process.env.SELLER_PUBLIC_BASE_URL!,
  amount: "50000", // USDC in microunits (e.g. 0.05 USDC)
});

// Boot capability lookup, cached enrichment template, and auto-refresher
await sdk.start();

const app = express();

// Apply the x402Middleware to any route you want to charge for
app.get("/api/premium", x402Middleware(sdk), (req, res) => {
  res.json({
    message: "Thank you for your payment! Here is your premium resource.",
    settlement: (req as any).payment,
  });
});
```

---

## File Layout

| Path | Purpose |
| --- | --- |
| `src/server.ts` | Express application setting up and starting the server. |
| `src/payment-required.ts` | Defines `X402SellerSDK` class and `x402Middleware` Express middleware. |
| `src/facilitator.ts` | `FacilitatorClient` for communicating with the verify/settle endpoints. |
