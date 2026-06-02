"""Wire-only sla-escrow accepts[] builder and SRM stub."""

from __future__ import annotations

import json
import os
from typing import Any


def _req(name: str) -> str:
    v = os.environ.get(name, "").strip()
    if not v:
        raise ValueError(f"missing {name}")
    return v


def sla_escrow_accepts_from_env() -> list[dict[str, Any]]:
    raw = os.environ.get("X402_ACCEPTS_JSON", "").strip()
    if raw:
        parsed = json.loads(raw)
        if not isinstance(parsed, list):
            raise ValueError("X402_ACCEPTS_JSON must be an array")
        return parsed

    merchant = (
        os.environ.get("X402_MERCHANT_WALLET")
        or os.environ.get("MERCHANT_WALLET")
        or os.environ.get("SELLER_WALLET")
        or ""
    )
    oracle_authorities = [
        s.strip()
        for s in os.environ.get("ORACLE_AUTHORITIES", "").split(",")
        if s.strip()
    ]
    profile_id = os.environ.get("ORACLE_PROFILE_ID", "x402/oracles/api-quality/v1")
    normative = os.environ.get("ORACLE_NORMATIVE_SPEC_URL", "")
    extra: dict[str, Any] = {
        "merchantWallet": merchant,
        "oracleAuthorities": oracle_authorities,
        "oracleProfiles": [{"profileId": profile_id, "normativeSpecUrl": normative}],
    }
    patch_raw = os.environ.get("X402_ACCEPTS_EXTRA_JSON", "").strip()
    if patch_raw:
        extra.update(json.loads(patch_raw))

    return [
        {
            "scheme": "sla-escrow",
            "network": _req("X402_NETWORK"),
            "asset": _req("X402_ASSET"),
            "amount": _req("X402_AMOUNT"),
            "payTo": _req("X402_PAY_TO"),
            "maxTimeoutSeconds": int(_req("X402_MAX_TIMEOUT_SECONDS")),
            "extra": extra,
        }
    ]


def accepts_for_env() -> list[dict[str, Any]]:
    if os.environ.get("X402_SCHEME", "exact") == "sla-escrow":
        return sla_escrow_accepts_from_env()
    from .payment_required import accepts_from_env

    return accepts_from_env()


def build_srm_json(paid_path: str) -> dict[str, Any]:
    base = os.environ.get("SELLER_PUBLIC_BASE_URL", "").rstrip("/")
    scheme = os.environ.get("X402_SCHEME", "exact")
    merchant = os.environ.get("X402_MERCHANT_WALLET") or os.environ.get("MERCHANT_WALLET") or ""
    path = paid_path if paid_path.startswith("/") else f"/{paid_path}"
    slug = path.lstrip("/").replace("/", "-")
    return {
        "schemaVersion": "0.1.0",
        "origin": base,
        "merchantWallet": merchant,
        "facilitatorHint": os.environ.get("FACILITATOR_BASE_URL"),
        "resources": [
            {
                "id": slug,
                "title": os.environ.get("SELLER_RESOURCE_DESCRIPTION", "Paid API"),
                "method": "GET",
                "resourceUrl": f"{base}{path}",
                "scheme": scheme,
                "tags": ["starter"],
            }
        ],
    }
