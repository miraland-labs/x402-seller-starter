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
from contextlib import asynccontextmanager
from typing import Any

from dotenv import load_dotenv
from fastapi import Depends, FastAPI

from .payment_required import (
    X402SellerSDK,
    register_x402_exception_handler,
    x402_payment_gate,
)


def _required_env(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        raise RuntimeError(f"missing required env var {name}")
    return value


def create_app() -> FastAPI:
    load_dotenv()

    paid_path = os.environ.get("SELLER_PAID_PATH", "/api/premium")
    free_path = os.environ.get("SELLER_FREE_PATH", "/api/free")

    # Initialize X402SellerSDK
    sdk = X402SellerSDK(
        facilitator_url=_required_env("FACILITATOR_BASE_URL"),
        seller_wallet=_required_env("MERCHANT_WALLET"),
        public_base_url=_required_env("SELLER_PUBLIC_BASE_URL"),
        amount=os.environ.get("X402_AMOUNT", "50000"),
        scheme=os.environ.get("X402_SCHEME", "exact"),
        asset=os.environ.get("X402_ASSET"),
        network=os.environ.get("X402_NETWORK"),
        max_timeout_seconds=int(os.environ.get("X402_MAX_TIMEOUT_SECONDS", "300")),
    )

    @asynccontextmanager
    async def lifespan(app: FastAPI):  # noqa: ARG001
        await sdk.start()
        yield
        await sdk.stop()

    app = FastAPI(
        title="x402-seller-starter-python",
        version="0.1.0",
        lifespan=lifespan,
    )

    # Register the custom exception handler for HTTP 402 responses
    register_x402_exception_handler(app)

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

    # Wrap routes with the dynamic X402 payment gate dependency
    @app.get(paid_path)
    async def paid_get(settlement: dict[str, Any] = Depends(x402_payment_gate(sdk))) -> dict[str, Any]:
        return {
            "tier": "paid",
            "message": "payment verified and settled",
            "settlement": settlement,
        }

    @app.post(paid_path)
    async def paid_post(settlement: dict[str, Any] = Depends(x402_payment_gate(sdk))) -> dict[str, Any]:
        return {
            "tier": "paid",
            "message": "payment verified and settled",
            "settlement": settlement,
        }

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
