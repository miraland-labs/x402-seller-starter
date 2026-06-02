"""Find sla-escrow payTo via facilitator discovery rail."""

from __future__ import annotations

import json
import os
import sys
import urllib.request


def main() -> None:
    base = os.environ.get("FACILITATOR_BASE_URL", "").rstrip("/")
    wallet = os.environ.get("MERCHANT_WALLET") or os.environ.get("SELLER_WALLET")
    asset = os.environ.get("X402_ASSET", "USDC")
    if not base or not wallet:
        print("Set FACILITATOR_BASE_URL and MERCHANT_WALLET", file=sys.stderr)
        sys.exit(1)
    url = f"{base}/api/v1/facilitator/sellers/{wallet}/rails/sla-escrow?asset={asset}"
    with urllib.request.urlopen(url) as resp:
        info = json.load(resp)
    pay_to = info.get("payTo")
    if not pay_to:
        print("discovery missing payTo", file=sys.stderr)
        sys.exit(1)
    print(f"X402_SCHEME=sla-escrow")
    print(f"X402_PAY_TO={pay_to}")
    print(f"X402_MERCHANT_WALLET={wallet}")


if __name__ == "__main__":
    main()
