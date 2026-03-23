/**
 * Example 04 — Error Handling and Configuration
 *
 * Demonstrates:
 *   - All HieroRuntimeError codes with explanations
 *   - retryable vs non-retryable classification
 *   - Custom retry config (maxAttempts, delays, jitter)
 *   - Custom finality config (timeouts, poll interval)
 *   - Config validation errors caught at createClient() time
 *   - Input validation errors caught at method call time
 *
 *   node packages/sdk/examples/04-error-handling.mjs
 */

import { createClient, HieroRuntimeError } from "../dist/index.js";
import { printSection, printError } from "./_utils.mjs";

// ── 1. Error code reference table ────────────────────────────────────────────

printSection("All HieroRuntimeError Codes");

const errorCodes = [
  {
    code: "INVALID_CONFIG",
    retryable: false,
    description: "Bad input at call time or bad createClient() options",
  },
  {
    code: "NOT_FOUND",
    retryable: false,
    description: "The requested resource (account, transaction, schedule) does not exist",
  },
  {
    code: "TIMEOUT",
    retryable: false,
    description: "Operation exceeded its configured timeout window",
  },
  {
    code: "TRANSPORT",
    retryable: true,
    description: "Network-level failure (connection refused, DNS, TLS)",
  },
  {
    code: "MIRROR_HTTP",
    retryable: true,
    description: "Mirror Node returned 5xx; may recover on retry",
  },
  {
    code: "RATE_LIMITED",
    retryable: true,
    description: "Mirror Node returned 429; SDK backs off automatically",
  },
  {
    code: "CONSENSUS",
    retryable: true,
    description: "Consensus node transient error (BUSY, PLATFORM_NOT_CREATED, EXPIRED)",
  },
  {
    code: "SCHEDULE",
    retryable: false,
    description: "Schedule operation failed (expired, deleted, or bad state)",
  },
  {
    code: "SERIALIZATION",
    retryable: false,
    description: "Mirror Node response had unexpected shape (possible API change)",
  },
  {
    code: "UNSUPPORTED",
    retryable: false,
    description: "Requested feature is not available for the selected network",
  },
  {
    code: "INTERNAL",
    retryable: false,
    description: "Unexpected internal error — please file a bug report",
  },
];

console.log(
  `  ${"Code".padEnd(20)} ${"Retryable".padEnd(12)} Description`,
);
console.log(`  ${"─".repeat(70)}`);
for (const { code, retryable, description } of errorCodes) {
  const r = retryable ? "yes" : "no";
  console.log(
    `  ${code.padEnd(20)} ${r.padEnd(12)} ${description}`,
  );
}

// ── 2. Trigger NOT_FOUND ──────────────────────────────────────────────────────

printSection("NOT_FOUND — Nonexistent account");

const client = await createClient({ network: "testnet" });

try {
  await client.mirror.accounts.get("0.0.99999999999");
} catch (err) {
  if (err instanceof HieroRuntimeError) {
    printError(err);
  }
}

// ── 3. Trigger INVALID_CONFIG at method call time ────────────────────────────

printSection("INVALID_CONFIG — Input validation");

const cases = [
  { label: "empty accountId", fn: () => client.mirror.accounts.get("") },
  {
    label: "empty transactionId",
    fn: () => client.mirror.transactions.get("   "),
  },
  {
    label: "empty schedule ID",
    fn: () => client.schedule.get(""),
  },
];

for (const { label, fn } of cases) {
  try {
    await fn();
  } catch (err) {
    if (err instanceof HieroRuntimeError) {
      console.log(`  ${label.padEnd(30)} → ${err.code}`);
    }
  }
}

// ── 4. Config validation at createClient() time ──────────────────────────────

printSection("INVALID_CONFIG — createClient() validation");

const configCases = [
  {
    label: "initialDelayMs > maxDelayMs",
    options: {
      network: "testnet",
      retry: { initialDelayMs: 5000, maxDelayMs: 100 },
    },
  },
  {
    label: "maxAttempts = 0",
    options: { network: "testnet", retry: { maxAttempts: 0 } },
  },
  {
    label: "empty operator.accountId",
    options: {
      network: "testnet",
      operator: { accountId: "  ", privateKey: "dummy" },
    },
  },
  {
    label: "custom without consensusNodes",
    options: {
      network: "custom",
      mirror: { baseUrl: "https://mirror.example.com" },
    },
  },
  {
    label: "custom without mirror.baseUrl",
    options: {
      network: "custom",
      consensusNodes: [{ url: "34.94.106.61:50211", accountId: "0.0.3" }],
    },
  },
];

for (const { label, options } of configCases) {
  try {
    await createClient(options);
    console.log(`  ${label.padEnd(40)} → (no error — unexpected)`);
  } catch (err) {
    if (err instanceof HieroRuntimeError) {
      console.log(`  ${label.padEnd(40)} → ${err.code}`);
    }
  }
}

// ── 5. Custom retry config ────────────────────────────────────────────────────
// Useful for: CLI tools (fast failure), batch jobs (aggressive retry),
// or latency-sensitive applications (minimal initial delay).

printSection("Custom Retry Configuration");

const fastFailClient = await createClient({
  network: "testnet",
  retry: {
    maxAttempts: 3,
    initialDelayMs: 50,
    maxDelayMs: 500,
    jitter: false, // deterministic delays — useful for tests
  },
});

console.log("  Fast-fail client config:");
console.log(`    maxAttempts:    ${fastFailClient.config.retry.maxAttempts}`);
console.log(`    initialDelayMs: ${fastFailClient.config.retry.initialDelayMs}`);
console.log(`    maxDelayMs:     ${fastFailClient.config.retry.maxDelayMs}`);
console.log(`    jitter:         ${fastFailClient.config.retry.jitter}`);

// Verify it works fine for normal operations
const account = await fastFailClient.mirror.accounts.get("0.0.98");
console.log(`  Query succeeded — account ${account.account} exists`);

// ── 6. Custom finality config ─────────────────────────────────────────────────
// Fine-tune how long to wait for consensus receipts and Mirror Node visibility.

printSection("Custom Finality Configuration");

const tightTimeoutClient = await createClient({
  network: "testnet",
  finality: {
    receiptTimeoutMs: 8_000, 
    mirrorTimeoutMs: 12_000, 
    pollIntervalMs: 200, 
  },
});

console.log("  Tight-timeout client config:");
console.log(
  `    receiptTimeoutMs: ${tightTimeoutClient.config.finality.receiptTimeoutMs}ms`,
);
console.log(
  `    mirrorTimeoutMs:  ${tightTimeoutClient.config.finality.mirrorTimeoutMs}ms`,
);
console.log(
  `    pollIntervalMs:   ${tightTimeoutClient.config.finality.pollIntervalMs}ms`,
);

console.log("\nDone.");
