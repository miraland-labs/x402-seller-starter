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
import { FacilitatorClient, FacilitatorError } from "./facilitator.js";
import {
  acceptsFromEnv,
  buildPaymentRequired,
  encodePaymentResponse,
  parsePaymentHeader,
  type PaymentRequired,
} from "./payment-required.js";

function required(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`missing required env var ${name}`);
  return v;
}

function fourOhTwoBody(pr: PaymentRequired, errorMessage: string): PaymentRequired {
  return { ...pr, error: errorMessage };
}

async function main() {
  // Fail fast at startup instead of at request time if env is incomplete.
  acceptsFromEnv();

  const paidPath = process.env["SELLER_PAID_PATH"] ?? "/api/premium";
  const freePath = process.env["SELLER_FREE_PATH"] ?? "/api/free";
  const facilitator = new FacilitatorClient(required("FACILITATOR_BASE_URL"));
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

  const paidHandler: express.RequestHandler = async (req, res) => {
    const pr = buildPaymentRequired(paidPath);

    // Header names are case-insensitive; Express lowercases them in `req.headers`.
    const raw = req.headers["payment-signature"];
    const rawStr = Array.isArray(raw) ? raw[0] : raw;
    if (!rawStr) {
      res.status(402).json(fourOhTwoBody(pr, "PAYMENT-SIGNATURE header is required (x402 v2)"));
      return;
    }

    let proof: Record<string, unknown>;
    try {
      proof = parsePaymentHeader(rawStr);
    } catch (e) {
      res.status(402).json(fourOhTwoBody(pr, `Invalid payment header: ${String(e)}`));
      return;
    }

    try {
      const settled = await facilitator.verifyAndSettle(proof);
      const message =
        typeof settled["settlementNote"] === "string"
          ? "payment verified; settlement already on-chain (idempotent)"
          : "payment verified and settled";
      res.setHeader("PAYMENT-RESPONSE", encodePaymentResponse(settled));
      res.json({ tier: "paid", message, settlement: settled });
    } catch (e) {
      const err = e instanceof FacilitatorError ? e : new Error(String(e));
      const errorResult = { success: false, errorReason: err.message };
      res.setHeader("PAYMENT-RESPONSE", encodePaymentResponse(errorResult));
      res.status(402).json(fourOhTwoBody(pr, `Facilitator: ${err.message}`));
    }
  };

  app.get(paidPath, paidHandler);
  app.post(paidPath, express.json({ limit: "32kb" }), paidHandler);

  app.listen(port, host, () => {
    console.log(
      `[x402-seller-starter-ts] listening on http://${host}:${port} free=${freePath} paid=${paidPath}`,
    );
  });
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
