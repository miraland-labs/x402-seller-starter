"""Compute ``X402_PAY_TO`` (vault PDA) for the exact rail.

PDA derivation matches the on-chain UniversalSettle program::

    find_program_address([b"vault", MERCHANT_WALLET], programId)

Also prints a ready-to-paste ``X402_ACCEPTS_EXTRA_JSON=...`` line pulled straight
from the facilitator's ``/supported`` kind.
"""

from __future__ import annotations

import json
import os
import sys

import httpx
from dotenv import load_dotenv
from solders.pubkey import Pubkey


def _trim_slash(s: str) -> str:
    return s.rstrip("/")


def main() -> None:
    load_dotenv()

    base_env = os.environ.get("FACILITATOR_BASE_URL", "").strip()
    if not base_env:
        print("Set FACILITATOR_BASE_URL in .env (or export it).", file=sys.stderr)
        sys.exit(1)
    base = _trim_slash(base_env)

    wallet = (os.environ.get("MERCHANT_WALLET") or os.environ.get("SELLER_WALLET") or "").strip()
    if not wallet:
        print(
            "Set MERCHANT_WALLET (or SELLER_WALLET) to your merchant base58 pubkey.",
            file=sys.stderr,
        )
        sys.exit(1)

    network = os.environ.get("X402_NETWORK", "solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp")

    res = httpx.get(f"{base}/api/v1/facilitator/supported", timeout=10.0)
    res.raise_for_status()
    supported = res.json()

    kinds = supported.get("kinds") or []
    exact = next(
        (k for k in kinds if k.get("scheme") == "exact" and k.get("network") == network),
        None,
    )
    if exact is None:
        print(
            f"No supported kind with scheme=exact and network={network!r}.\n"
            f"  curl -sS {base}/api/v1/facilitator/supported | jq .kinds",
            file=sys.stderr,
        )
        sys.exit(1)

    extra = exact.get("extra") or {}
    program_id_str = extra.get("programId")
    if not isinstance(program_id_str, str):
        print("supported kind missing extra.programId", file=sys.stderr)
        sys.exit(1)

    merchant = Pubkey.from_string(wallet)
    program_id = Pubkey.from_string(program_id_str)
    vault_pda, _bump = Pubkey.find_program_address([b"vault", bytes(merchant)], program_id)

    print()
    print("═══ find_payto — seller needs this for 402 / .env ═══")
    print()
    print(f"Facilitator:     {base}")
    print(f"MERCHANT_WALLET: {wallet}")
    print(f"X402_NETWORK:    {network}")
    print(f"programId:       {program_id_str}")
    print()
    print("╔════════════════════════════════════════════════════════════════════════╗")
    print("║  payTo  (vault PDA for v2:solana:exact — NOT your merchant wallet)   ║")
    print("╠════════════════════════════════════════════════════════════════════════╣")
    print(f"║  {vault_pda}")
    print("╚════════════════════════════════════════════════════════════════════════╝")
    # Inject merchantWallet into extra so the facilitator can always resolve the real
    # seller identity, even before the vault is activated on-chain (JIT provision path).
    extra_with_merchant = dict(extra)
    if "merchantWallet" not in extra_with_merchant:
        extra_with_merchant["merchantWallet"] = wallet

    print()
    print(f"X402_PAY_TO={vault_pda}")
    print()
    print("Includes `merchantWallet` so the facilitator resolves your identity correctly.")
    print("Paste into .env (single-quoted so inner \" are fine):")
    print(f"X402_ACCEPTS_EXTRA_JSON='{json.dumps(extra_with_merchant, separators=(',', ':'))}'")
    print()
    print("Next: put X402_PAY_TO + X402_ACCEPTS_EXTRA_JSON in .env, then: x402-seller-start")


if __name__ == "__main__":  # pragma: no cover
    main()
