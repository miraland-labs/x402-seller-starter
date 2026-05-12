// Compute X402_PAY_TO (vault PDA) for the exact rail:
//   find_program_address(["vault", MERCHANT_WALLET], programId)
//
// Also prints an X402_ACCEPTS_EXTRA_JSON=... line pulled straight from the
// facilitator's /supported kind so the env.example placeholders can be filled
// in with one copy-paste.

import "dotenv/config";
import { PublicKey } from "@solana/web3.js";

type SupportedKind = {
  scheme: string;
  network: string;
  extra?: Record<string, unknown>;
};

type SupportedDoc = {
  kinds?: SupportedKind[];
};

function trimTrailingSlash(s: string): string {
  return s.replace(/\/+$/, "");
}

async function main() {
  const base = trimTrailingSlash(
    process.env["FACILITATOR_BASE_URL"] ??
      (() => {
        throw new Error("Set FACILITATOR_BASE_URL in .env (or export it).");
      })(),
  );
  const wallet =
    process.env["MERCHANT_WALLET"] ??
    process.env["SELLER_WALLET"] ??
    (() => {
      throw new Error("Set MERCHANT_WALLET (or SELLER_WALLET) to your merchant base58 pubkey.");
    })();
  const network =
    process.env["X402_NETWORK"] ?? "solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp";

  const res = await fetch(`${base}/api/v1/facilitator/supported`);
  if (!res.ok) {
    throw new Error(`GET /supported failed: ${res.status} ${await res.text()}`);
  }
  const supported = (await res.json()) as SupportedDoc;
  const kinds = supported.kinds ?? [];
  const exact = kinds.find(
    (k) => k.scheme === "exact" && k.network === network,
  );
  if (!exact) {
    throw new Error(
      `No supported kind with scheme=exact and network=${network}. Try:\n` +
        `  curl -sS ${base}/api/v1/facilitator/supported | jq .kinds`,
    );
  }
  const programIdStr = exact.extra?.["programId"];
  if (typeof programIdStr !== "string") {
    throw new Error("supported kind missing extra.programId");
  }

  const merchant = new PublicKey(wallet.trim());
  const programId = new PublicKey(programIdStr);
  const [vaultPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), merchant.toBuffer()],
    programId,
  );

  console.log();
  console.log("═══ find-payto — seller needs this for 402 / .env ═══");
  console.log();
  console.log(`Facilitator:     ${base}`);
  console.log(`MERCHANT_WALLET: ${wallet}`);
  console.log(`X402_NETWORK:    ${network}`);
  console.log(`programId:       ${programIdStr}`);
  console.log();
  console.log("╔════════════════════════════════════════════════════════════════════════╗");
  console.log("║  payTo  (vault PDA for v2:solana:exact — NOT your merchant wallet)   ║");
  console.log("╠════════════════════════════════════════════════════════════════════════╣");
  console.log(`║  ${vaultPda.toBase58()}`);
  console.log("╚════════════════════════════════════════════════════════════════════════╝");
  console.log();
  console.log(`X402_PAY_TO=${vaultPda.toBase58()}`);
  console.log();

  if (exact.extra) {
    // Inject merchantWallet into extra so the facilitator can always resolve the real
    // seller identity, even before the vault is activated on-chain (JIT provision path).
    const extraWithMerchant = { ...exact.extra };
    if (!("merchantWallet" in extraWithMerchant)) {
      extraWithMerchant["merchantWallet"] = wallet.trim();
    }
    console.log("Includes `merchantWallet` so the facilitator resolves your identity correctly.");
    console.log("Paste into .env (single-quoted so inner \" are fine):");
    console.log(`X402_ACCEPTS_EXTRA_JSON='${JSON.stringify(extraWithMerchant)}'`);
    console.log();
  }

  console.log("Next: put X402_PAY_TO + X402_ACCEPTS_EXTRA_JSON in .env, then: npm start");
}

main().catch((err) => {
  console.error(`Error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
