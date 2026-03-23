/**
 * Example 05 — HBAR Transfer with Two-Phase Finality
 *
 * Demonstrates:
 *   - createClient() with an operator (required for write operations)
 *   - submit.hbarTransfer() — submit the transaction, get a tx ID immediately
 *   - finality.wait() — polls consensus receipt, then waits for Mirror visibility
 *   - client.attach() — build a handle for an existing transaction ID
 *   - finality.waitForReceipt() — just the consensus layer, no Mirror polling
 *
 * REQUIRES credentials set as environment variables:
 *   export HEDERA_NETWORK=testnet        # testnet | mainnet | previewnet (default: testnet)
 *
 *   export HEDERA_OPERATOR_ID=0.0.12345
 *   export HEDERA_OPERATOR_KEY=3030...
 *   export HEDERA_RECEIVER_ID=0.0.67890   # optional, defaults to 0.0.98
 *
 *   node packages/sdk/examples/05-hbar-transfer.mjs
 *
 * Get free testnet credentials at https://portal.hedera.com
 */

import { createClient, HieroRuntimeError } from "../dist/index.js";
import { tinybarsToHbar, printSection, printError, formatTimestamp } from "./_utils.mjs";

const network = process.env.HEDERA_NETWORK ?? "testnet";
const operatorId = process.env.HEDERA_OPERATOR_ID;
const operatorKey = process.env.HEDERA_OPERATOR_KEY;
const receiverId = process.env.HEDERA_RECEIVER_ID ?? "0.0.98";

if (!operatorId || !operatorKey) {
  console.log("HBAR Transfer example requires credentials.\n");
  console.log("Set these environment variables, then re-run:");
  console.log("  export HEDERA_NETWORK=testnet          # or mainnet / previewnet");
  console.log("  export HEDERA_OPERATOR_ID=0.0.<your-account>");
  console.log("  export HEDERA_OPERATOR_KEY=3030...     (your ECDSA private key, DER-encoded)");
  console.log("  export HEDERA_RECEIVER_ID=0.0.<receiver>  # optional\n");
  console.log("Free testnet accounts: https://portal.hedera.com");
  process.exit(0);
}

const AMOUNT_TINYBAR = "1000000"; // 0.01 HBAR 

// ── Setup ─────────────────────────────────────────────────────────────────────

console.log(`  Network: ${network}`);

const client = await createClient({
  network,
  operator: {
    accountId: operatorId,
    privateKey: operatorKey,
  },
  // Tighter finality polling 
  finality: {
    receiptTimeoutMs: 15_000,
    mirrorTimeoutMs: 20_000,
    pollIntervalMs: 300,
  },
});

printSection("HBAR Transfer");

console.log(`  From:        ${operatorId}`);
console.log(`  To:          ${receiverId}`);
console.log(`  Amount:      ${BigInt(AMOUNT_TINYBAR).toLocaleString("en-US")} tinybar`);
console.log(`               ${tinybarsToHbar(AMOUNT_TINYBAR)}`);

// ── Step 1: Submit the transaction ────────────────────────────────────────────
// submit.hbarTransfer() returns immediately with a transaction ID.
// The transaction has been submitted to the consensus network but is not yet confirmed. 

console.log("\n  Submitting...");
const t0 = performance.now();

let submitted;
try {
  submitted = await client.submit.hbarTransfer({
    fromAccountId: operatorId,
    toAccountId: receiverId,
    amountTinybar: AMOUNT_TINYBAR,
  });
} catch (err) {
  console.log("\n  Submission failed:");
  printError(err);
  process.exit(1);
}

console.log(`  Transaction ID: ${submitted.transactionId}`);
console.log("  (transaction submitted — waiting for finality...)");

// ── Step 2: Wait for two-phase finality ───────────────────────────────────────
// finality.wait() does two things in sequence:
//   Phase 1 — Polls consensus nodes until a receipt is available
//   Phase 2 — Polls Mirror Node until the transaction record is visible
//
// This is unique to this SDK — @hashgraph/sdk only gives the receipt.
// Mirror Node visibility means transaction is queryable by any observer.

let finalized;
try {
  finalized = await client.finality.wait(submitted.transactionId);
} catch (err) {
  console.log("\n  Finality wait failed:");
  printError(err);
  process.exit(1);
}

const elapsed = ((performance.now() - t0) / 1000).toFixed(1);

printSection("Finality Result");

console.log(`  Status:        ${finalized.receipt.status}`);
console.log(`  Transaction:   ${finalized.transactionId}`);
console.log(`  Time to finality: ${elapsed}s`);

if (finalized.primaryMirrorEntry) {
  const tx = finalized.primaryMirrorEntry;
  console.log(`\n  Mirror entry:`);
  console.log(`    Type:        ${tx.name ?? "(unknown)"}`);
  console.log(`    Result:      ${tx.result}`);
  console.log(`    Timestamp:   ${formatTimestamp(tx.consensusTimestamp)}`);
  console.log(`    Scheduled:   ${tx.scheduled ?? false}`);
}

if (finalized.duplicates.length > 0) {
  console.log(`\n  Duplicate entries: ${finalized.duplicates.length}`);
}

// ── Step 3: Receipt-only (no Mirror polling) ──────────────────────────────────
// Use finality.waitForReceipt() when you only care about consensus confirmation
// and don't need the full Mirror Node record. It is faster than finality.wait().

printSection("Receipt-Only Wait (finality.waitForReceipt)");

console.log("  Submitting a second small transfer...");

let submitted2;
try {
  submitted2 = await client.submit.hbarTransfer({
    fromAccountId: operatorId,
    toAccountId: receiverId,
    amountTinybar: AMOUNT_TINYBAR,
  });
} catch (err) {
  console.log("  Submission failed:");
  printError(err);
  process.exit(1);
}

const t1 = performance.now();
let receipt;
try {
  receipt = await client.finality.waitForReceipt(submitted2.transactionId);
} catch (err) {
  console.log("  Receipt wait failed:");
  printError(err);
  process.exit(1);
}

const receiptTime = ((performance.now() - t1) / 1000).toFixed(1);

console.log(`  Status:  ${receipt.status}`);
console.log(`  TX ID:   ${receipt.transactionId}`);
console.log(`  Time:    ${receiptTime}s  (receipt only — no Mirror polling)`);

// ── Step 4: client.attach() pattern ──────────────────────────────────────────
// If you already have a transaction ID from another system (e.g. submitted by
// a different SDK, stored in a database, passed via an API), use attach() to
// create a handle that can wait for finality without resubmitting.

printSection("client.attach() — Track an Existing Transaction");

const handle = client.attach(submitted.transactionId);

console.log(`  Attached to: ${handle.transactionId}`);
console.log("  (transaction already finalized — Mirror will return immediately)");

const reFinalized = await handle.waitForFinality();
console.log(`  Status:  ${reFinalized.receipt.status}`);
console.log(`  Mirror:  ${reFinalized.primaryMirrorEntry ? "visible" : "not yet visible"}`);

console.log("\nDone.");
