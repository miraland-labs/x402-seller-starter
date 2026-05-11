// Build the x402 v2 Payment Required JSON body from environment variables.
// Mirrors the Rust starter's `accepts_from_env` + `build_payment_required`.

export type AcceptsRow = {
  scheme: string;
  network: string;
  asset: string;
  amount: string;
  payTo: string;
  maxTimeoutSeconds: number;
  extra?: Record<string, unknown>;
};

export type PaymentRequired = {
  x402Version: 2;
  error?: string;
  resource: {
    url: string;
    description: string;
    mimeType: string;
  };
  accepts: AcceptsRow[];
  // `extensions` is the x402 v2 spec's escape hatch for non-standard hints.
  // Namespace the key with `pr402` so a buyer sees unambiguously which
  // facilitator implementation this 402 is pointing at.
  extensions: { pr402FacilitatorUrl: string };
};

function required(name: string): string {
  const v = process.env[name];
  if (!v || v.trim() === "") {
    throw new Error(
      `missing required env var \`${name}\`. Set X402_ACCEPTS_JSON or all of: X402_SCHEME, X402_NETWORK, X402_ASSET, X402_AMOUNT, X402_PAY_TO, X402_MAX_TIMEOUT_SECONDS.`,
    );
  }
  return v;
}

function trimTrailingSlash(s: string): string {
  return s.replace(/\/+$/, "");
}

export function acceptsFromEnv(): AcceptsRow[] {
  // Precedence: X402_ACCEPTS_JSON wins over discrete vars.
  const fullJson = process.env["X402_ACCEPTS_JSON"]?.trim();
  if (fullJson) {
    const parsed: unknown = JSON.parse(fullJson);
    if (!Array.isArray(parsed)) {
      throw new Error("X402_ACCEPTS_JSON must be a JSON array.");
    }
    return parsed as AcceptsRow[];
  }

  const maxTimeoutRaw = required("X402_MAX_TIMEOUT_SECONDS");
  const maxTimeoutSeconds = Number.parseInt(maxTimeoutRaw, 10);
  if (!Number.isFinite(maxTimeoutSeconds) || maxTimeoutSeconds < 0) {
    throw new Error(`X402_MAX_TIMEOUT_SECONDS must be a non-negative integer: ${maxTimeoutRaw}`);
  }

  const row: AcceptsRow = {
    scheme: required("X402_SCHEME"),
    network: required("X402_NETWORK"),
    asset: required("X402_ASSET"),
    amount: required("X402_AMOUNT"),
    payTo: required("X402_PAY_TO"),
    maxTimeoutSeconds,
  };

  const extraRaw = process.env["X402_ACCEPTS_EXTRA_JSON"]?.trim();
  if (extraRaw) {
    row.extra = JSON.parse(extraRaw) as Record<string, unknown>;
  }
  return [row];
}

export function buildPaymentRequired(resourcePath: string): PaymentRequired {
  const publicBase = trimTrailingSlash(required("SELLER_PUBLIC_BASE_URL"));
  const facilitatorBase = trimTrailingSlash(required("FACILITATOR_BASE_URL"));
  const description = process.env["SELLER_RESOURCE_DESCRIPTION"] ?? "Premium seller API route";
  const mimeType = process.env["SELLER_RESOURCE_MIME"] ?? "application/json";
  const path = resourcePath.startsWith("/") ? resourcePath : `/${resourcePath}`;
  return {
    x402Version: 2,
    resource: { url: `${publicBase}${path}`, description, mimeType },
    accepts: acceptsFromEnv(),
    extensions: { pr402FacilitatorUrl: facilitatorBase },
  };
}

// Accepts raw UTF-8 JSON (the interop default) or base64-of-that-JSON.
export function parsePaymentHeader(raw: string): Record<string, unknown> {
  const trimmed = raw.trim();
  try {
    return JSON.parse(trimmed) as Record<string, unknown>;
  } catch {
    const decoded = Buffer.from(trimmed, "base64").toString("utf8");
    return JSON.parse(decoded) as Record<string, unknown>;
  }
}

// Encode a settle result for the `PAYMENT-RESPONSE` response header.
export function encodePaymentResponse(settleResult: unknown): string {
  return Buffer.from(JSON.stringify(settleResult), "utf8").toString("base64");
}
