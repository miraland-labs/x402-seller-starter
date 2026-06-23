// Example Express server: free route, paid route with HTTP 402 + PAYMENT-SIGNATURE
// settlement via pr402. x402 v2 wire flow:
//
//   Server -> Client: HTTP 402 with Payment Required JSON in the response body.
//                     (The spec also defines an optional PAYMENT-REQUIRED response
//                      header; this example does not emit it — the JSON body is
//                      the interop default and what pr402 buyers read.)
//   Client -> Server: PAYMENT-SIGNATURE request header carrying the signed proof.
//   Server -> Client: PAYMENT-RESPONSE response header (base64 JSON of the settle
//                     result), emitted on both HTTP 200 and HTTP 402 outcomes.
//
// Run: `cp .env.example .env` then `npm install && npm start`.

import "dotenv/config";
import express from "express";
import {
  X402SellerSDK,
  x402Middleware,
} from "./payment-required.js";

function required(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`missing required env var ${name}`);
  return v;
}

async function main() {
  const paidPath = process.env["SELLER_PAID_PATH"] ?? "/api/premium";
  const freePath = process.env["SELLER_FREE_PATH"] ?? "/api/free";

  // Initialize X402SellerSDK with dynamic capabilities derivation and background enrich caching
  const sdk = new X402SellerSDK({
    facilitatorUrl: required("FACILITATOR_BASE_URL"),
    sellerWallet: required("MERCHANT_WALLET"),
    publicBaseUrl: required("SELLER_PUBLIC_BASE_URL"),
    amount: process.env["X402_AMOUNT"] ?? "50000",
    scheme: process.env["X402_SCHEME"] || "exact",
    asset: process.env["X402_ASSET"],
    network: process.env["X402_NETWORK"],
    maxTimeoutSeconds: process.env["X402_MAX_TIMEOUT_SECONDS"]
      ? parseInt(process.env["X402_MAX_TIMEOUT_SECONDS"], 10)
      : undefined,
  });

  await sdk.start();

  const bind = process.env["BIND_ADDR"] ?? "127.0.0.1:3000";
  const [host = "127.0.0.1", portStr = "3000"] = bind.split(":");
  const port = Number.parseInt(portStr, 10);
  if (!Number.isFinite(port)) throw new Error(`invalid BIND_ADDR port: ${bind}`);

  const app = express();
  app.disable("x-powered-by");

  app.get("/", (_req, res) => {
    res.json({
      service: "x402-seller-starter-ts",
      free: freePath,
      paid: paidPath,
      facilitator: required("FACILITATOR_BASE_URL"),
      docs: "https://github.com/miraland-labs/x402",
    });
  });

  app.get(freePath, (_req, res) => {
    res.json({ tier: "free", message: "no payment required" });
  });

  app.get(paidPath, x402Middleware(sdk), (req, res) => {
    res.json({
      tier: "paid",
      message: "payment verified and settled",
      settlement: (req as any).payment,
    });
  });

  app.post(
    paidPath,
    express.json({ limit: "32kb" }),
    x402Middleware(sdk),
    (req, res) => {
      res.json({
        tier: "paid",
        message: "payment verified and settled",
        settlement: (req as any).payment,
      });
    }
  );

  app.listen(port, host, () => {
    console.log(
      `[x402-seller-starter-ts] listening on http://${host}:${port} free=${freePath} paid=${paidPath}`
    );
  });
}


main().catch((err) => {
  console.error(err);
  process.exit(1);
});
