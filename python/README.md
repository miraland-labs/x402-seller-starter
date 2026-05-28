# x402-seller-starter · Python

Reference Python seller for [pr402](https://github.com/miralandlabs/pr402) / [x402 v2](https://github.com/coinbase/x402/blob/main/specs/x402-specification-v2.md). About 250 lines total: a FastAPI app that gates one route with HTTP 402 and settles via a pr402 facilitator.

## Scope

Targets the **`exact`** (UniversalSettle) rail — instant micro-payments. For `sla-escrow`, see the [pr402 SLA docs](https://ipay.sh/onboarding_guide.md).

## Quick start

Requires Python ≥ 3.11.

```bash
cp .env.example .env           # fill in MERCHANT_WALLET
python -m venv .venv
source .venv/bin/activate       # Windows: .venv\Scripts\activate
pip install -e .

x402-seller-find-payto          # computes X402_PAY_TO + prints X402_ACCEPTS_EXTRA_JSON
# paste both lines back into .env
x402-seller-start               # starts uvicorn on BIND_ADDR (default 127.0.0.1:3000)
```

Then:

```bash
curl -sS http://127.0.0.1:3000/api/free
curl -i  http://127.0.0.1:3000/api/premium
curl -sS http://127.0.0.1:3000/api/premium \
  -H "PAYMENT-SIGNATURE: $(cat proof.json)"
```

A valid `PAYMENT-SIGNATURE` is the same JSON body you'd POST to facilitator `/verify` — build it via your buyer agent / the pr402 `build-exact-payment-tx` endpoint.

## Layout

| Path                                                | Purpose                                                   |
| --------------------------------------------------- | --------------------------------------------------------- |
| `src/x402_seller_starter/server.py`                 | FastAPI app: free route + paid route (GET/POST).          |
| `src/x402_seller_starter/payment_required.py`       | Build the x402 v2 Payment Required body from env vars.    |
| `src/x402_seller_starter/facilitator.py`            | `FacilitatorClient.verify_and_settle` — async httpx.       |
| `src/x402_seller_starter/find_payto.py`             | Compute `payTo` + dump `X402_ACCEPTS_EXTRA_JSON`.         |

## Before serving your first 402

1. Put the output of `x402-seller-find-payto` into `X402_PAY_TO` and `X402_ACCEPTS_EXTRA_JSON`.
2. **Activate your vault on-chain** at [ipay.sh](https://ipay.sh) (or `POST /api/v1/facilitator/sellers/provision-tx`). Skipping this step still yields valid 402 responses, but settle returns `409 Conflict — vault not yet on-chain`.

## Notes

- `parse_payment_header` accepts raw JSON and base64-of-JSON.
- `verify_and_settle` raises `FacilitatorError` for HTTP-200-but-`isValid: false` proofs, so you can surface `invalidReason` to buyers.
- The 402 JSON body carries `extensions.pr402FacilitatorUrl` as a discoverability hint.
