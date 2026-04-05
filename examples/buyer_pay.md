# Buyer flow (reference)

This file mirrors the README **Buyer agent checklist** for easy sharing.

## Actors

- **Buyer** — builds and signs the payment, sends `X-PAYMENT` to the **seller**.
- **Seller** — forwards the same JSON to the facilitator **verify** then **settle**, then returns the resource.
- **Facilitator (pr402)** — builds unsigned tx, verifies proofs, submits settlement.

## Steps

1. Call the paid route without payment → **402** + payment requirements.
2. Call facilitator **build** for your rail (e.g. `build-exact-payment-tx`) with `payer`, `accepted` (one `accepts[]` line), `resource`.
3. Sign the returned transaction at `payerSignatureIndex`.
4. Put the signed base64 tx into the verify body (same shape as facilitator **verify**).
5. **Retry the seller** with header: `X-PAYMENT: <that JSON as one line or standard header encoding>`.

Do **not** call facilitator **settle** from the buyer if the seller will **settle** — that runs settlement twice.

If you see blockhash errors, call **build** again and sign immediately.

## Seller-starter behavior

`FacilitatorClient::verify_and_settle` calls **verify** then **settle**. If **settle** fails with an “already processed” on-chain message but **verify** was valid, the client returns a **synthetic success** JSON (with `settlementNote`) so paid access still works on older or flaky retry paths.
