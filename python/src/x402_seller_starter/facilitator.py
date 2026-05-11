"""Minimal pr402 verify + settle client for x402 v2 sellers (Python).

Same shape as the Rust and TypeScript starters: two sequential POSTs with the
same JSON body; surfaces ``isValid: false`` (HTTP-200 semantic failure) as a
``FacilitatorError`` so the seller can forward ``invalidReason`` to buyers;
keeps a legacy fallback for facilitators that haven't normalized duplicate
on-chain settle into HTTP 200.
"""

from __future__ import annotations

import json
from typing import Any, Literal

import httpx


class FacilitatorError(RuntimeError):
    def __init__(
        self,
        message: str,
        step: Literal["verify", "settle"],
        status: int | None = None,
        body: str | None = None,
    ) -> None:
        super().__init__(message)
        self.step = step
        self.status = status
        self.body = body


class FacilitatorClient:
    """Verify+settle against a pr402 facilitator."""

    def __init__(self, facilitator_base: str, *, timeout: float = 20.0) -> None:
        base = facilitator_base.rstrip("/")
        self._verify_url = f"{base}/api/v1/facilitator/verify"
        self._settle_url = f"{base}/api/v1/facilitator/settle"
        self._timeout = timeout

    async def verify_and_settle(self, body: dict[str, Any]) -> dict[str, Any]:
        async with httpx.AsyncClient(timeout=self._timeout) as client:
            verify_res = await client.post(self._verify_url, json=body)
            verify_text = verify_res.text
            if verify_res.status_code >= 400:
                raise FacilitatorError(
                    f"verify {verify_res.status_code}: {verify_text[:400]}",
                    "verify",
                    verify_res.status_code,
                    verify_text,
                )
            try:
                verify_json = verify_res.json()
            except json.JSONDecodeError as exc:
                raise FacilitatorError(
                    f"verify response not JSON: {verify_text[:200]}",
                    "verify",
                    verify_res.status_code,
                    verify_text,
                ) from exc

            # `isValid: false` is not an HTTP error — the facilitator returns
            # HTTP 200 with `{isValid: false, invalidReason: "..."}` for
            # semantically invalid proofs (wrong amount, wrong payee, ...).
            is_valid = verify_json.get("isValid") is True or verify_json.get("valid") is True
            if not is_valid:
                raise FacilitatorError(
                    f"verify indicated invalid proof: {verify_text[:400]}",
                    "verify",
                    verify_res.status_code,
                    verify_text,
                )

            settle_body = dict(body)
            cid = verify_json.get("correlationId")
            if isinstance(cid, str) and cid and "correlationId" not in settle_body:
                settle_body["correlationId"] = cid

            settle_res = await client.post(self._settle_url, json=settle_body)
            settle_text = settle_res.text
            if settle_res.status_code >= 400:
                if _is_duplicate_settle_body(settle_text):
                    return _synthetic_settlement_after_duplicate(
                        verify_json, body, settle_text
                    )
                raise FacilitatorError(
                    f"settle {settle_res.status_code}: {settle_text[:400]}",
                    "settle",
                    settle_res.status_code,
                    settle_text,
                )
            try:
                return settle_res.json()
            except json.JSONDecodeError as exc:
                raise FacilitatorError(
                    f"settle response not JSON: {settle_text[:200]}",
                    "settle",
                    settle_res.status_code,
                    settle_text,
                ) from exc


def _is_duplicate_settle_body(body: str) -> bool:
    lower = body.lower()
    return (
        "already been processed" in lower
        or "alreadyprocessed" in lower
        or "this transaction has already been processed" in lower
    )


def _synthetic_settlement_after_duplicate(
    verify: dict[str, Any],
    proof: dict[str, Any],
    settle_error_snippet: str,
) -> dict[str, Any]:
    network = ""
    reqs = proof.get("paymentRequirements")
    if isinstance(reqs, dict):
        network_val = reqs.get("network")
        if isinstance(network_val, str):
            network = network_val
    return {
        "success": True,
        "payer": verify.get("payer"),
        "network": network,
        "transaction": "",
        "settlementNote": (
            "verify succeeded; settle reported duplicate on-chain — treating as "
            "idempotent success"
        ),
        "settleErrorPreview": settle_error_snippet[:240],
    }
