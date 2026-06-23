# x402-seller-starter · Python

Reference Python seller for [pr402](https://github.com/miralandlabs/pr402) / [x402 v2](https://github.com/coinbase/x402/blob/main/specs/x402-specification-v2.md). It implements the `X402SellerSDK` class and a FastAPI route dependency gate that intercepts requests, gates them with HTTP 402, and settles payments via a pr402 facilitator.

## Scope

Targets the **`exact`** (UniversalSettle) rail — instant micro-payments.

## Quick start

Requires Python ≥ 3.11.

1. **Configure environment**: Copy `.env.example` to `.env` and fill in:
   - `FACILITATOR_BASE_URL` (e.g. `https://preview.ipay.sh`)
   - `MERCHANT_WALLET` (your public wallet address)
   - `SELLER_PUBLIC_BASE_URL` (your server's public URL, e.g. `http://127.0.0.1:3000`)
   - `X402_AMOUNT` (USDC price per request, in decimals)

2. **Onboard**: Follow the [x402-cli README](https://github.com/miraland-labs/x402/blob/main/tools/x402-cli/README.md) to activate your vault on-chain and register your merchant wallet off-chain.

3. **Install & Run**:
   ```bash
   python -m venv .venv
   source .venv/bin/activate
   pip install -e .
   x402-seller-start
   ```

4. **Verify**:
   ```bash
   curl -sS http://127.0.0.1:3000/api/free
   curl -i  http://127.0.0.1:3000/api/premium                # Returns HTTP 402 with enriched challenge JSON
   ```

---

## Code Usage

You can easily integrate this into your existing FastAPI application:

```python
from fastapi import FastAPI, Depends
from x402_seller_starter.payment_required import (
    X402SellerSDK,
    x402_payment_gate,
    register_x402_exception_handler
)

# 1. Initialize the SDK
sdk = X402SellerSDK(
    facilitator_url="https://preview.ipay.sh",
    seller_wallet="your_wallet_address",
    public_base_url="http://127.0.0.1:3000",
    amount="50000", # USDC microunits (e.g. 0.05 USDC)
)

app = FastAPI()

# 2. Register exception handler to parse custom X402PaymentRequiredException
register_x402_exception_handler(app)

@app.on_event("startup")
async def startup():
    # Start capability lookup and background poll cache refresher
    await sdk.start()

@app.on_event("shutdown")
async def shutdown():
    await sdk.stop()

# 3. Add x402_payment_gate as a FastAPI dependency
@app.get("/api/premium")
async def premium_route(settlement=Depends(x402_payment_gate(sdk))):
    return {
        "message": "Thank you for your payment! Here is your premium resource.",
        "settlement": settlement
    }
```

---

## File Layout

| Path | Purpose |
| --- | --- |
| `src/x402_seller_starter/server.py` | FastAPI application setting up and starting the server. |
| `src/x402_seller_starter/payment_required.py` | Defines `X402SellerSDK` class, FastAPI exception handlers, and the gate dependency. |
| `src/x402_seller_starter/facilitator.py` | `FacilitatorClient` for communicating with the verify/settle endpoints. |
