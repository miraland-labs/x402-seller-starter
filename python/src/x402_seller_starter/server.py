"""Example FastAPI server: free route + paid route with HTTP 402 + PAYMENT-SIGNATURE.

x402 v2 wire flow this server implements:

- Server -> Client: HTTP 402 with the payment-required JSON in the **response body**.
  The spec also defines an optional ``PAYMENT-REQUIRED`` response header; this
  example does not emit it — the JSON body is the interop default.
- Client -> Server: ``PAYMENT-SIGNATURE`` request header carrying the signed proof.
- Server -> Client: ``PAYMENT-RESPONSE`` response header (base64 JSON of the
  settle result), emitted on both HTTP 200 and HTTP 402 outcomes.

Run locally::

    cp .env.example .env
    pip install -e .
    x402-seller-start      # or: python -m x402_seller_starter.server
"""

from __future__ import annotations

import os
from typing import Any

from dotenv import load_dotenv
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse

from .facilitator import FacilitatorClient, FacilitatorError
from .payment_required import (
    accepts_from_env,
    build_payment_required,
    encode_payment_response,
    parse_payment_header,
)


def _required_env(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        raise RuntimeError(f"missing required env var {name}")
    return value


def _four_oh_two(body: dict[str, Any], error: str) -> dict[str, Any]:
    return {**body, "error": error}


def create_app() -> FastAPI:
    load_dotenv()

    # Fail fast at startup rather than at request time if env is incomplete.
    accepts_from_env()

    paid_path = os.environ.get("SELLER_PAID_PATH", "/api/premium")
    free_path = os.environ.get("SELLER_FREE_PATH", "/api/free")
    facilitator = FacilitatorClient(_required_env("FACILITATOR_BASE_URL"))

    app = FastAPI(title="x402-seller-starter-python", version="0.1.0")

    @app.get("/")
    async def root() -> dict[str, Any]:
        return {
            "service": "x402-seller-starter-python",
            "free": free_path,
            "paid": paid_path,
            "facilitator": _required_env("FACILITATOR_BASE_URL"),
            "docs": "https://github.com/miraland-labs/x402",
        }

    @app.get(free_path)
    async def free() -> dict[str, Any]:
        return {"tier": "free", "message": "no payment required"}

    async def paid_handler(request: Request) -> JSONResponse:
        pr = build_payment_required(paid_path)

        # HTTP header names are case-insensitive; Starlette normalizes to lower.
        raw = request.headers.get("payment-signature")
        if not raw:
            return JSONResponse(
                status_code=402,
                content=_four_oh_two(pr, "PAYMENT-SIGNATURE header is required (x402 v2)"),
            )

        try:
            proof = parse_payment_header(raw)
        except Exception as exc:  # noqa: BLE001 — surface the parse error verbatim
            return JSONResponse(
                status_code=402,
                content=_four_oh_two(pr, f"Invalid payment header: {exc}"),
            )

        try:
            settled = await facilitator.verify_and_settle(proof)
        except FacilitatorError as exc:
            error_result = {"success": False, "errorReason": str(exc)}
            return JSONResponse(
                status_code=402,
                content=_four_oh_two(pr, f"Facilitator: {exc}"),
                headers={"PAYMENT-RESPONSE": encode_payment_response(error_result)},
            )

        message = (
            "payment verified; settlement already on-chain (idempotent)"
            if "settlementNote" in settled
            else "payment verified and settled"
        )
        return JSONResponse(
            content={"tier": "paid", "message": message, "settlement": settled},
            headers={"PAYMENT-RESPONSE": encode_payment_response(settled)},
        )

    # GET and POST share the same gate.
    app.add_api_route(paid_path, paid_handler, methods=["GET", "POST"])

    return app


def main() -> None:
    """Console-script entrypoint (see ``[project.scripts]`` in pyproject.toml)."""

    import uvicorn

    load_dotenv()
    bind = os.environ.get("BIND_ADDR", "127.0.0.1:3000")
    host, _, port_str = bind.partition(":")
    port = int(port_str) if port_str else 3000
    uvicorn.run(
        "x402_seller_starter.server:create_app",
        host=host or "127.0.0.1",
        port=port,
        factory=True,
        reload=False,
    )


if __name__ == "__main__":  # pragma: no cover
    main()
