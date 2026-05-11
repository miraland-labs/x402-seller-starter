"""x402 v2 Payment Required body builder + header helpers (seller side).

Mirrors the Rust and TypeScript starters. Keeps zero Solana dependencies in the
HTTP layer — only the ``find_payto`` companion uses ``solders`` for PDA math.
"""

from __future__ import annotations

import base64
import json
import os
from typing import Any


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
