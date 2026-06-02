import type { Request, Response } from "express";

export function slaEscrowAcceptsFromEnv(): Record<string, unknown>[] {
  const raw = process.env["X402_ACCEPTS_JSON"];
  if (raw?.trim()) {
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) throw new Error("X402_ACCEPTS_JSON must be an array");
    return parsed as Record<string, unknown>[];
  }
  const req = (name: string): string => {
    const v = process.env[name];
    if (!v?.trim()) throw new Error(`missing ${name}`);
    return v;
  };
  const merchant =
    process.env["X402_MERCHANT_WALLET"] ||
    process.env["MERCHANT_WALLET"] ||
    process.env["SELLER_WALLET"] ||
    "";
  const oracleAuthorities = (process.env["ORACLE_AUTHORITIES"] || "")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
  const profileId =
    process.env["ORACLE_PROFILE_ID"] || "x402/oracles/api-quality/v1";
  const normative = process.env["ORACLE_NORMATIVE_SPEC_URL"] || "";
  let extra: Record<string, unknown> = {
    merchantWallet: merchant,
    oracleAuthorities,
    oracleProfiles: [{ profileId, normativeSpecUrl: normative }],
  };
  if (process.env["X402_ACCEPTS_EXTRA_JSON"]?.trim()) {
    extra = {
      ...extra,
      ...(JSON.parse(process.env["X402_ACCEPTS_EXTRA_JSON"]!) as Record<
        string,
        unknown
      >),
    };
  }
  return [
    {
      scheme: "sla-escrow",
      network: req("X402_NETWORK"),
      asset: req("X402_ASSET"),
      amount: req("X402_AMOUNT"),
      payTo: req("X402_PAY_TO"),
      maxTimeoutSeconds: Number.parseInt(req("X402_MAX_TIMEOUT_SECONDS"), 10),
      extra,
    },
  ];
}

export function acceptsForEnv(): Record<string, unknown>[] {
  const scheme = process.env["X402_SCHEME"] || "exact";
  if (scheme === "sla-escrow") return slaEscrowAcceptsFromEnv();
  const { acceptsFromEnv } = require("./payment-required.js") as {
    acceptsFromEnv: () => Record<string, unknown>[];
  };
  return acceptsFromEnv();
}

export function buildSrmJson(paidPath: string): Record<string, unknown> {
  const base = (process.env["SELLER_PUBLIC_BASE_URL"] || "").replace(/\/$/, "");
  const scheme = process.env["X402_SCHEME"] || "exact";
  const merchant =
    process.env["X402_MERCHANT_WALLET"] ||
    process.env["MERCHANT_WALLET"] ||
    "";
  const path = paidPath.startsWith("/") ? paidPath : `/${paidPath}`;
  return {
    schemaVersion: "0.1.0",
    origin: base,
    merchantWallet: merchant,
    facilitatorHint: process.env["FACILITATOR_BASE_URL"],
    resources: [
      {
        id: path.replace(/^\//, "").replace(/\//g, "-"),
        title: process.env["SELLER_RESOURCE_DESCRIPTION"] || "Paid API",
        method: "GET",
        resourceUrl: `${base}${path}`,
        scheme,
        tags: ["starter"],
      },
    ],
  };
}

export function srmHandler(paidPath: string) {
  return (_req: Request, res: Response) => {
    res.json(buildSrmJson(paidPath));
  };
}
