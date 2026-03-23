/**
 * Example 03 — Single Transaction Lookup
 *
 * Demonstrates:
 *   - mirror.transactions.get() for a specific transaction ID
 *   - Primary record vs duplicate records (duplicate transactions on Hedera)
 *   - Transaction ID format: base, ?scheduled, ?nonce=N suffixes
 *   - requestedTransactionId vs actual entry IDs in the response
 *
 * Fetches a real transaction ID dynamically by first
 * listing the most recent transaction for account 0.0.98.
 *
 *   node packages/sdk/examples/03-transaction-lookup.mjs
 *  
 * Override the network (default: testnet):
 *   HEDERA_NETWORK=mainnet node packages/sdk/examples/03-transaction-lookup.mjs
 */

import { createClient, HieroRuntimeError } from "../dist/index.js";
import { printSection, printError, formatTimestamp } from "./_utils.mjs";

const network = process.env.HEDERA_NETWORK ?? "testnet";
const client = await createClient({ network });

// ── 1. Get a real transaction ID from the treasury account ────────────────────

printSection("Fetching a Recent Transaction ID");

const page = await client.mirror.transactions.list("0.0.98", { limit: 1 });
if (page.items.length === 0) {
  console.log("  No transactions found — try again later.");
  process.exit(0);
}

const recentTx = page.items[0];
const txId = recentTx.transactionId;
console.log(`  Found transaction: ${txId}`);
console.log(`  Type:              ${recentTx.name ?? "(unknown)"}`);
console.log(`  Result:            ${recentTx.result}`);
console.log(`  Timestamp:         ${formatTimestamp(recentTx.consensusTimestamp)}`);

// ── 2. Look up that transaction by ID ─────────────────────────────────────────
// The Mirror Node can return multiple records for a single transaction ID when
// duplicate transactions exist. The SDK selects the correct "primary" record
// based on the requested ID (plain, ?scheduled, or ?nonce=N).

printSection("mirror.transactions.get() — Full Lookup");

const lookup = await client.mirror.transactions.get(txId);

console.log(`  Requested ID:      ${lookup.requestedTransactionId}`);
console.log();
console.log("  Primary record:");
console.log(`    Transaction ID:  ${lookup.primary.transactionId}`);
console.log(`    Result:          ${lookup.primary.result}`);
console.log(`    Name:            ${lookup.primary.name ?? "(unknown)"}`);
console.log(`    Timestamp:       ${formatTimestamp(lookup.primary.consensusTimestamp)}`);
console.log(`    Scheduled:       ${lookup.primary.scheduled ?? false}`);
console.log(`    Nonce:           ${lookup.primary.nonce ?? "(none)"}`);
console.log();
console.log(`  Total entries:     ${lookup.entries.length}`);
console.log(`  Duplicate count:   ${lookup.duplicates.length}`);

if (lookup.duplicates.length > 0) {
  console.log();
  console.log("  Duplicate records:");
  for (const dup of lookup.duplicates) {
    console.log(`    - ${dup.transactionId}  result=${dup.result}`);
  }
}

// ── 3. Transaction ID suffix formats ─────────────────────────────────────────
// The SDK understands three ID formats:
//   "0.0.1001@1234567890.000000001"           → plain
//   "0.0.1001@1234567890.000000001?scheduled" → scheduled transaction execution
//   "0.0.1001@1234567890.000000001?nonce=2"   → duplicate with nonce

printSection("Transaction ID Suffix Formats");

const scheduledId = `${txId}?scheduled`;
console.log(`  Attempting scheduled lookup: ${scheduledId}`);
try {
  const scheduledLookup = await client.mirror.transactions.get(scheduledId);
  console.log(`  Found! Primary result: ${scheduledLookup.primary.result}`);
  console.log(`  scheduled flag: ${scheduledLookup.primary.scheduled}`);
} catch (err) {
  if (err instanceof HieroRuntimeError && err.code === "NOT_FOUND") {
    console.log("  → NOT_FOUND (expected — this is not a scheduled transaction)");
    console.log(
      "  The ?scheduled suffix filters for the execution record of a",
    );
    console.log("  ScheduleCreateTransaction, not the schedule itself.");
  } else {
    printError(err);
  }
}

console.log();
console.log("  The ?nonce=N suffix selects among duplicate transaction entries.");
console.log("  Useful when a transaction was submitted multiple times with");
console.log("  different nonces (e.g. in retry scenarios).");
console.log(`  Example: ${txId}?nonce=0`);

console.log("\nDone.");
