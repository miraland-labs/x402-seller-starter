"""x402 v2 Payment Required body builder + header helpers (seller side).

Mirrors the Rust and TypeScript starters. Keeps zero Solana dependencies in the
HTTP layer — only the ``find_payto`` companion uses ``solders`` for PDA math.
"""

from __future__ import annotations

import asyncio
import base64
import json
import os
from typing import Any, Optional

import httpx
from fastapi import Request as FARequest, Response as FAResponse
from fastapi.responses import JSONResponse

from .facilitator import FacilitatorClient


def _required(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        raise RuntimeError(
            f"missing required env var `{name}`. Set X402_ACCEPTS_JSON or all of: "
            "X402_SCHEME, X402_NETWORK, X402_ASSET, X402_AMOUNT, X402_PAY_TO, "
            "X402_MAX_TIMEOUT_SECONDS."
        )
    return value


def _trim_slash(s: str) -> str:
    return s.rstrip("/")


def accepts_from_env() -> list[dict[str, Any]]:
    """Build the ``accepts`` array.

    Precedence: ``X402_ACCEPTS_JSON`` (a full array literal) wins over discrete
    vars. ``X402_ACCEPTS_EXTRA_JSON`` is merged into the single discrete-row
    when the former is absent.
    """

    full_json = os.environ.get("X402_ACCEPTS_JSON", "").strip()
    if full_json:
        parsed = json.loads(full_json)
        if not isinstance(parsed, list):
            raise RuntimeError("X402_ACCEPTS_JSON must be a JSON array.")
        return parsed

    max_timeout_raw = _required("X402_MAX_TIMEOUT_SECONDS")
    try:
        max_timeout = int(max_timeout_raw)
    except ValueError as exc:
        raise RuntimeError(
            f"X402_MAX_TIMEOUT_SECONDS must be a non-negative integer: {max_timeout_raw!r}"
        ) from exc
    if max_timeout < 0:
        raise RuntimeError("X402_MAX_TIMEOUT_SECONDS must be non-negative.")

    row: dict[str, Any] = {
        "scheme": _required("X402_SCHEME"),
        "network": _required("X402_NETWORK"),
        "asset": _required("X402_ASSET"),
        "amount": _required("X402_AMOUNT"),
        "payTo": _required("X402_PAY_TO"),
        "maxTimeoutSeconds": max_timeout,
    }

    extra_raw = os.environ.get("X402_ACCEPTS_EXTRA_JSON", "").strip()
    if extra_raw:
        row["extra"] = json.loads(extra_raw)

    # Inject merchantWallet into extra if MERCHANT_WALLET is set and extra doesn't already have it.
    # This ensures the facilitator can always resolve the real seller identity from the 402 body,
    # even when the vault PDA doesn't exist on-chain yet (pre-activation / JIT provision).
    merchant_wallet = (
        os.environ.get("MERCHANT_WALLET", "").strip()
        or os.environ.get("SELLER_WALLET", "").strip()
    )
    if merchant_wallet:
        extra = row.get("extra")
        if isinstance(extra, dict) and "merchantWallet" not in extra:
            extra["merchantWallet"] = merchant_wallet
        elif extra is None:
            row["extra"] = {"merchantWallet": merchant_wallet}

    return [row]


def build_payment_required(resource_path: str) -> dict[str, Any]:
    """Assemble the x402 v2 Payment Required JSON body for a protected route."""

    public_base = _trim_slash(_required("SELLER_PUBLIC_BASE_URL"))
    facilitator_base = _trim_slash(_required("FACILITATOR_BASE_URL"))
    description = os.environ.get(
        "SELLER_RESOURCE_DESCRIPTION", "Premium seller API route"
    )
    mime = os.environ.get("SELLER_RESOURCE_MIME", "application/json")
    path = resource_path if resource_path.startswith("/") else "/" + resource_path
    return {
        "x402Version": 2,
        "resource": {
            "url": f"{public_base}{path}",
            "description": description,
            "mimeType": mime,
        },
        "accepts": accepts_from_env(),
        # `extensions` is the x402 v2 spec's escape hatch. Namespace the key with
        # `pr402` so a buyer sees unambiguously which facilitator implementation
        # this 402 is pointing at.
        "extensions": {"pr402FacilitatorUrl": facilitator_base},
    }


def parse_payment_header(raw: str) -> dict[str, Any]:
    """Parse a ``PAYMENT-SIGNATURE`` header value.

    Raw UTF-8 JSON is the interop default; base64-of-that-JSON is also accepted.
    """

    trimmed = raw.strip()
    try:
        return json.loads(trimmed)
    except json.JSONDecodeError:
        decoded = base64.b64decode(trimmed).decode("utf-8")
        return json.loads(decoded)


def encode_payment_response(settle_result: Any) -> str:
    """Encode a settle result as a base64 string for the ``PAYMENT-RESPONSE`` header."""

    return base64.b64encode(
        json.dumps(settle_result, separators=(",", ":")).encode("utf-8")
    ).decode("ascii")




def _four_oh_two(body: dict[str, Any], error: str) -> dict[str, Any]:
    return {**body, "error": error}

class X402SellerSDK:
    """Unified client-side seller SDK supporting dynamic capability resolution and cached enrichment."""

    def __init__(
        self,
        facilitator_url: str,
        seller_wallet: str,
        public_base_url: str,
        amount: str,
        scheme: str = "exact",
        asset: Optional[str] = None,
        network: Optional[str] = None,
        max_timeout_seconds: int = 300,
    ) -> None:
        self.facilitator_url = facilitator_url.rstrip("/")
        self.seller_wallet = seller_wallet
        self.public_base_url = public_base_url.rstrip("/")
        self.amount = amount
        self.scheme = scheme
        self.asset = asset
        self.network = network
        self.max_timeout_seconds = max_timeout_seconds
        self.cached_body: Optional[dict[str, Any]] = None
        self.cache_ttl_seconds = 600  # 10 minutes
        self.facilitator_client = FacilitatorClient(self.facilitator_url)
        self.refresh_task: Optional[asyncio.Task] = None

    async def start(self) -> None:
        """Start the SDK by resolving capabilities, cache-enriching, and spawning refresher."""
        await self._refresh_cache()
        self.refresh_task = asyncio.create_task(self._background_refresher())

    async def stop(self) -> None:
        """Stop the background refresher task."""
        if self.refresh_task:
            self.refresh_task.cancel()
            try:
                await self.refresh_task
            except asyncio.CancelledError:
                pass
            self.refresh_task = None

    async def _background_refresher(self) -> None:
        while True:
            await asyncio.sleep(self.cache_ttl_seconds)
            try:
                await self._refresh_cache()
            except Exception as e:
                print(f"[X402SellerSDK] Background cache refresh failed: {e}")

    async def _refresh_cache(self) -> None:
        async with httpx.AsyncClient(timeout=10.0) as client:
            resolved_network = self.network
            resolved_asset = self.asset

            # Dynamic capabilities lookup
            if not resolved_network or not resolved_asset:
                res = await client.get(f"{self.facilitator_url}/api/v1/facilitator/capabilities")
                res.raise_for_status()
                caps = res.json()
                if not resolved_network:
                    resolved_network = (
                        caps.get("solanaNetwork")
                        or caps.get("network")
                        or "solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp"
                    )
                if not resolved_asset:
                    resolved_asset = (
                        caps.get("usdcMint") or "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
                    )

            # Construct naive draft for facilitator-enrich API
            draft = {
                "x402Version": 2,
                "resource": {"url": f"{self.public_base_url}/api/placeholder"},
                "accepts": [
                    {
                        "scheme": self.scheme,
                        "network": resolved_network,
                        "payTo": self.seller_wallet,
                        "asset": resolved_asset,
                        "amount": self.amount,
                        "maxTimeoutSeconds": self.max_timeout_seconds,
                    }
                ],
            }

            res = await client.post(
                f"{self.facilitator_url}/api/v1/facilitator/payment-required/enrich",
                json=draft,
            )
            res.raise_for_status()
            self.cached_body = res.json()

    def get_payment_required(self, resource_path: str) -> dict[str, Any]:
        """Return the fully enriched payment required body with dynamic requested URL path."""
        if not self.cached_body:
            raise RuntimeError("X402SellerSDK is not initialized. Call start() first.")
        path = resource_path if resource_path.startswith("/") else "/" + resource_path
        body = dict(self.cached_body)
        body["resource"] = dict(body["resource"])
        body["resource"]["url"] = f"{self.public_base_url}{path}"
        return body

    async def verify_and_settle(self, proof: dict[str, Any]) -> dict[str, Any]:
        """Forward payment signature proof to facilitator verify+settle endpoints."""
        return await self.facilitator_client.verify_and_settle(proof)


class X402PaymentRequiredException(Exception):
    """Exception raised by x402 gate dependency to yield clean HTTP 402 responses."""

    def __init__(
        self, payment_required_body: dict[str, Any], headers: Optional[dict[str, str]] = None
    ) -> None:
        super().__init__()
        self.payment_required_body = payment_required_body
        self.headers = headers or {}


def register_x402_exception_handler(app: Any) -> None:
    """Register custom exception handler on FastAPI application to format 402 responses."""

    @app.exception_handler(X402PaymentRequiredException)
    async def x402_exception_handler(request: Any, exc: X402PaymentRequiredException) -> JSONResponse:
        return JSONResponse(
            status_code=402, content=exc.payment_required_body, headers=exc.headers
        )


def x402_payment_gate(sdk: X402SellerSDK) -> Any:
    """FastAPI dynamic dependency gate returning settlement context or raising 402."""

    async def dependency(request: FARequest, response: FAResponse) -> dict[str, Any]:
        raw = request.headers.get("payment-signature")
        path = request.url.path
        if request.url.query:
            path += f"?{request.url.query}"
        pr = sdk.get_payment_required(path)

        if not raw:
            raise X402PaymentRequiredException(
                _four_oh_two(pr, "PAYMENT-SIGNATURE header is required (x402 v2)")
            )

        try:
            proof = parse_payment_header(raw)
        except Exception as exc:
            raise X402PaymentRequiredException(
                _four_oh_two(pr, f"Invalid payment header: {exc}")
            )

        try:
            settled = await sdk.verify_and_settle(proof)
            response.headers["PAYMENT-RESPONSE"] = encode_payment_response(settled)
            return settled
        except Exception as exc:
            error_result = {"success": False, "errorReason": str(exc)}
            raise X402PaymentRequiredException(
                _four_oh_two(pr, f"Facilitator: {exc}"),
                headers={"PAYMENT-RESPONSE": encode_payment_response(error_result)},
            )

    return dependency

