# x402-seller-starter · TypeScript

Reference TypeScript seller for [pr402](https://github.com/miralandlabs/pr402) / [x402 v2](https://github.com/coinbase/x402/blob/main/specs/x402-specification-v2.md). About 200 lines total: an Express server that gates one route with HTTP 402 and settles via a pr402 facilitator.

## Scope

Targets the **`exact`** (UniversalSettle) rail — instant micro-payments. For `sla-escrow`, see the [pr402 SLA docs](https://ipay.sh/onboarding_guide.md).

## Quick start

Requires Node.js ≥ 20 (uses the built-in `fetch`, ESM, and top-level `await` where convenient).

```bash
cp .env.example .env         # fill in MERCHANT_WALLET
npm install
npm run find-payto           # computes X402_PAY_TO + prints X402_ACCEPTS_EXTRA_JSON
# paste both lines back into .env
npm start                    # starts Express on BIND_ADDR (default 127.0.0.1:3000)
```

Then:

```bash
curl -sS  http://127.0.0.1:3000/api/free
curl -i   http://127.0.0.1:3000/api/premium                 # HTTP 402
curl -sS  http://127.0.0.1:3000/api/premium \
  -H "PAYMENT-SIGNATURE: $(cat proof.json)"                  # HTTP 200 on valid proof
```

A valid `PAYMENT-SIGNATURE` is the same JSON body you'd POST to facilitator `/verify` — build it via your buyer agent / the pr402 `build-exact-payment-tx` endpoint.

## Layout

| Path                          | Purpose                                                           |
| ----------------------------- | ----------------------------------------------------------------- |
| `src/server.ts`               | Express app: free route + paid route (GET/POST).                  |
| `src/payment-required.ts`     | Build the x402 v2 Payment Required body from env vars.            |
| `src/facilitator.ts`          | `FacilitatorClient.verifyAndSettle` — two POSTs + error mapping.  |
| `src/find-payto.ts`           | Compute `payTo` (vault PDA) + dump `X402_ACCEPTS_EXTRA_JSON`.     |

## Before serving your first 402

1. Put the output of `npm run find-payto` into `X402_PAY_TO` and `X402_ACCEPTS_EXTRA_JSON`.
2. **Activate your vault on-chain** at [ipay.sh](https://ipay.sh) (or `POST /api/v1/facilitator/onboard/provision`). Skipping this step still yields valid 402 responses, but settle returns `409 Conflict — vault not yet on-chain`.

## Notes

- Header names are normalized to lowercase before lookup. `parsePaymentHeader` accepts raw JSON and base64-of-JSON.
- `verifyAndSettle` surfaces `isValid: false` (HTTP-200 semantic failure) as a `FacilitatorError`, so you can forward `invalidReason` to buyers.
- The 402 JSON body carries `extensions.pr402FacilitatorUrl` as a discoverability hint; buyers that don't recognize the key should ignore it.
