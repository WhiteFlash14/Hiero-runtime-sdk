/**
 * Example 01 — Mirror Node Account Lookup
 *
 * Demonstrates:
 *   - createClient() with no operator (Mirror-only, read-only)
 *   - mirror.accounts.get()
 *   - Balance displayed in both tinybar and HBAR
 *   - Structured HieroRuntimeError with .code and .retryable
 *
 * Override the network (default: testnet):
 *   HEDERA_NETWORK=mainnet node packages/sdk/examples/01-account-lookup.mjs
 */

import { createClient, HieroRuntimeError } from "../dist/index.js";
import { tinybarsToHbar, printSection, printError } from "./_utils.mjs";

const network = process.env.HEDERA_NETWORK ?? "testnet";
const client = await createClient({ network });

// ── Query the Hedera treasury account ─────────────────────────────────────
// Account 0.0.98 is the Hedera treasury — guaranteed to exist with a large
// balance and rich transaction history on every network.

printSection("Mirror Account Lookup — 0.0.98 (Hedera Treasury)");

const treasury = await client.mirror.accounts.get("0.0.98");

console.log(`  Account:     ${treasury.account}`);
console.log(`  Balance:     ${BigInt(treasury.balance).toLocaleString("en-US")} tinybar`);
console.log(`               ${tinybarsToHbar(treasury.balance)}`);
console.log(`  EVM addr:    ${treasury.evmAddress ?? "(none)"}`);
console.log(`  Deleted:     ${treasury.deleted}`);
console.log(`  Memo:        ${treasury.memo || "(empty)"}`);

// ── 2. Query another well-known system account ────────────────────────────────

printSection("Mirror Account Lookup — 0.0.800 (Staking Reward Account)");

const stakingReward = await client.mirror.accounts.get("0.0.800");
console.log(`  Account:     ${stakingReward.account}`);
console.log(`  Balance:     ${tinybarsToHbar(stakingReward.balance)}`);
console.log(`  Memo:        ${stakingReward.memo || "(empty)"}`);

// ── 3. Demonstrate NOT_FOUND error ────────────────────────────────────────────
// This shows how HieroRuntimeError works exactly as described in the TypeScript types.

printSection("Error Handling — NOT_FOUND");

console.log("  Querying account 0.0.99999999999 (does not exist)...");
try {
  await client.mirror.accounts.get("0.0.99999999999");
} catch (err) {
  if (err instanceof HieroRuntimeError) {
    printError(err);
    console.log();
    console.log("  This error is NOT retryable — a 404 means the account");
    console.log("  genuinely does not exist, not a transient failure.");
  }
}

// ── 4. Demonstrate INVALID_CONFIG error ──────────────────────────────────────
// Input validation happens in TypeScript before hitting the network, so it
// fails fast with a clear error message.

printSection("Error Handling — INVALID_CONFIG");

console.log("  Calling mirror.accounts.get with an empty string...");
try {
  await client.mirror.accounts.get("  ");
} catch (err) {
  if (err instanceof HieroRuntimeError) {
    printError(err);
  }
}

console.log("\nDone.");
