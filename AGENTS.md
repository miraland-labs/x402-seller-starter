# AGENTS.md

This file is for AI agents (Cursor, Claude Code, etc.), not human developers.
Philosophy: **Simple is Best, yet Elegant.** Make the smallest change that solves
the task; do not refactor, abstract, or add features that were not asked for.

`x402-seller-starter` is a **reference** seller in three languages. Each starter gates
one HTTP route with `402 Payment Required` and settles via a pr402 facilitator —
nothing more. It is meant to be read and copied, not grown into a framework.

## Topology

- `rust/`        — Axum, `cargo run --example axum_server` (+ `find_payto`, `find_escrow_payto`).
- `typescript/`  — Express 5, Node ≥ 20, `npm start`.
- `python/`      — FastAPI, Python ≥ 3.11, `x402-seller-start`.

No root-level build. Each directory is self-contained.

## Hard boundaries (do not cross without explicit human approval)

- **Keep the three languages in lockstep.** Same feature set, same env var names, same
  `402` body shape on the wire. If you change one language, change all three or change none.
- **Env var names are a contract.** `SELLER_PUBLIC_BASE_URL`, `FACILITATOR_BASE_URL`,
  `MERCHANT_WALLET`, `X402_SCHEME`, `X402_NETWORK`, `X402_ASSET`, `X402_AMOUNT`,
  `X402_PAY_TO`, `X402_MAX_TIMEOUT_SECONDS`, `X402_ACCEPTS_EXTRA_JSON`. Don't rename.
- **Scope = gate one route + the two-POST `verify`/`settle` loop.** Don't add a registry,
  delivery, DB, or UI. `sla-escrow` (`X402_SCHEME=sla-escrow`) stays wire-only.
- **Authoritative payment terms = the live HTTP 402.** `/.well-known/x402-resources.json`
  is advisory discovery metadata only.
- **No new dependencies** (`Cargo.toml`, `package.json`, `pyproject.toml`) unless asked.

## Verify before claiming done (fix, don't suppress)

```bash
cd rust       && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo build --examples
cd typescript && npm install && npx tsc          # tsconfig has noEmit:true → type-check only
cd python     && python -m pip install -e . && python -c "import x402_seller_starter"
```
