/**
 * Example 06 — Scheduled Transfer Lifecycle
 *
 * Demonstrates:
 *   - schedule.createTransfer() — wrap a transfer in a ScheduleCreateTransaction
 *   - schedule.get()            — query current status and list of signatories
 *   - schedule.sign()           — add a co-signature (multi-party signing)
 *   - schedule.wait()           — poll until execution, return finalized tx
 *   - schedule.delete()         — cancel a pending schedule
 *
 * Scheduled transactions are one of the most powerful and unique features of
 * Hedera. They allow multiple parties to agree on a future action without any
 * party having to hold another's private key.
 *
 * REQUIRES credentials:
 *
 *   export HEDERA_NETWORK=testnet                    # testnet | mainnet | previewnet (default: testnet)
 *   export HEDERA_OPERATOR_ID=0.0.12345
 *   export HEDERA_OPERATOR_KEY=3030...
 *   export HEDERA_RECEIVER_ID=0.0.67890              # optional, defaults to 0.0.98
 *
 * Optional — enables the real multi-sig path:
 *   export HEDERA_SECOND_SIGNER_ACCOUNT_ID=0.0.67890
 *   export HEDERA_SECOND_SIGNER_KEY=3030...
 *
 *   node packages/sdk/examples/06-scheduled-transfer.mjs
 *
 * Get free testnet credentials at https://portal.hedera.com
 */

import { createClient, HieroRuntimeError } from "../dist/index.js";
import {
  tinybarsToHbar,
  printSection,
  printError,
  formatTimestamp,
} from "./_utils.mjs";

const network = process.env.HEDERA_NETWORK ?? "testnet";
const operatorId = process.env.HEDERA_OPERATOR_ID;
const operatorKey = process.env.HEDERA_OPERATOR_KEY;
const receiverId = process.env.HEDERA_RECEIVER_ID ?? "0.0.98";
const secondSignerKey = process.env.HEDERA_SECOND_SIGNER_KEY;

if (!operatorId || !operatorKey) {
  console.log("Scheduled Transfer example requires credentials.\n");
  console.log("Set these environment variables, then re-run:");
  console.log("  export HEDERA_NETWORK=testnet              # or mainnet / previewnet");
  console.log("  export HEDERA_OPERATOR_ID=0.0.<your-account>");
  console.log("  export HEDERA_OPERATOR_KEY=3030...         (your ECDSA private key, DER-encoded)");
  console.log("  export HEDERA_RECEIVER_ID=0.0.<receiver>   # optional\n");
  console.log("Optional — real multi-sig path:");
  console.log("  export HEDERA_SECOND_SIGNER_ACCOUNT_ID=0.0.<second-account>");
  console.log("  export HEDERA_SECOND_SIGNER_KEY=3030...\n");
  console.log("Free testnet accounts: https://portal.hedera.com");
  process.exit(0);
}

const AMOUNT_TINYBAR = "500000"; // 0.005 HBAR

const secondSignerAccountId = process.env.HEDERA_SECOND_SIGNER_ACCOUNT_ID;
const fromAccountId = (secondSignerKey && secondSignerAccountId) ? secondSignerAccountId : operatorId;
const payerAccountId = (secondSignerKey && secondSignerAccountId) ? operatorId : undefined;

const client = await createClient({
  network,
  operator: { accountId: operatorId, privateKey: operatorKey },
  finality: { receiptTimeoutMs: 15_000, mirrorTimeoutMs: 20_000, pollIntervalMs: 300 },
});

// ── 1. Create a scheduled transfer ───────────────────────────────────────────
// The operator wraps a HBAR transfer inside a ScheduleCreateTransaction.
// The underlying transfer is NOT executed yet — it is stored on-ledger
// and waits until all required signatories have signed.
//
// When fromAccountId == operator, the operator's signature is automatically
// included. For a standard single-key account this executes immediately.

printSection("Create Scheduled Transfer");

console.log(`  From:    ${fromAccountId}${payerAccountId ? " (sender)" : ""}`);
if (payerAccountId) console.log(`  Payer:   ${payerAccountId} (fee payer)`);
console.log(`  To:      ${receiverId}`);
console.log(`  Amount:  ${tinybarsToHbar(AMOUNT_TINYBAR)}`);
console.log(`  Memo:    "demo scheduled payment"`);
if (secondSignerKey && secondSignerAccountId) {
  console.log(`\n  Multi-sig mode: sender is ${fromAccountId}, operator pays fees`);
  console.log(`  Expected status: pendingSignatures (sender key not yet added)`);
} else {
  console.log(`\n  Single-sig mode: operator is sender, auto-executes on creation`);
}
console.log("\n  Submitting ScheduleCreateTransaction...");

let created;
try {
  created = await client.schedule.createTransfer({
    fromAccountId,
    toAccountId: receiverId,
    amountTinybar: AMOUNT_TINYBAR,
    payerAccountId,
    memo: `demo scheduled payment ${Date.now()}`,
  });
} catch (err) {
  console.log("\n  Creation failed:");
  printError(err);
  process.exit(1);
}

console.log(`\n  Schedule ID:    ${created.scheduleId}`);
console.log(`  Scheduled TX:   ${created.scheduledTransactionId}`);
console.log(`  Initial status: ${created.status}`);

// ── 2. Query the schedule state ───────────────────────────────────────────────
// schedule.get() calls ScheduleInfoQuery on the consensus network.
// It returns the full schedule record: signatories, expiry time, creator, etc.

printSection("Query Schedule State — schedule.get()");

let info;
try {
  info = await client.schedule.get(created.scheduleId);
} catch (err) {
  printError(err);
  process.exit(1);
}

console.log(`  Schedule ID:     ${info.scheduleId}`);
console.log(`  Status:          ${info.status}`);
console.log(`  Creator:         ${info.creatorAccountId ?? "(unknown)"}`);
console.log(`  Payer:           ${info.payerAccountId ?? "(same as creator)"}`);
console.log(`  Signatories:     ${info.signatories.length} key(s)`);
for (const key of info.signatories) {
  console.log(`    - ${key.slice(0, 24)}...`);
}
if (info.expirationTime) {
  console.log(`  Expires:         ${formatTimestamp(info.expirationTime)}`);
}
if (info.executedTimestamp) {
  console.log(`  Executed at:     ${formatTimestamp(info.executedTimestamp)}`);
}

// ── 3a. Already executed (single-sig path) ────────────────────────────────────
if (info.status === "executed") {
  printSection("Schedule Already Executed (single-signer)");
  console.log("  The operator's signature was sufficient to execute the schedule");
  console.log("  immediately on creation. This is the expected behaviour for a");
  console.log("  standard single-key account.");
  console.log("\n  Calling schedule.wait() to retrieve the finalized record...");

  let execution;
  try {
    execution = await client.schedule.wait(created.scheduleId);
  } catch (err) {
    printError(err);
    process.exit(1);
  }

  printSection("Execution Result");
  console.log(`  Schedule ID:     ${execution.scheduleId}`);
  console.log(`  Scheduled TX:    ${execution.scheduledTransactionId}`);
  console.log(`  TX status:       ${execution.finalized.receipt.status}`);
  if (execution.finalized.primaryMirrorEntry) {
    console.log(`  Timestamp:       ${formatTimestamp(execution.finalized.primaryMirrorEntry.consensusTimestamp)}`);
    console.log(`  Mirror result:   ${execution.finalized.primaryMirrorEntry.result}`);
  }
}

// ── 3b. Pending — optional second signer ─────────────────────────────────────
// This path is reached when the schedule stays in pendingSignatures.
// In production, multiple parties call schedule.sign() independently from
// their own wallets — no party ever sees another's private key.
if (info.status === "pendingSignatures") {
  printSection("Schedule Pending — Waiting for Signatures");
  console.log("  Status: pendingSignatures");
  console.log("  The schedule is waiting for additional signatures.");

  if (secondSignerKey) {
    console.log(`\n  HEDERA_SECOND_SIGNER_KEY provided — signing now...`);

    let updatedInfo;
    try {
      updatedInfo = await client.schedule.sign({
        scheduleId: created.scheduleId,
        signerPrivateKey: secondSignerKey,
      });
    } catch (err) {
      console.log("  Signing failed:");
      printError(err);
      process.exit(1);
    }

    console.log(`  Status after signing: ${updatedInfo.status}`);
    console.log(`  Signatories now:      ${updatedInfo.signatories.length}`);

    if (updatedInfo.status === "executed" || updatedInfo.status === "pendingSignatures") {
      console.log("\n  Waiting for execution...");
      try {
        const execution = await client.schedule.wait(created.scheduleId);
        console.log(`  TX status: ${execution.finalized.receipt.status}`);
      } catch (err) {
        printError(err);
      }
    }
  } else {
    console.log("\n  To demonstrate multi-sig signing:");
    console.log("    export HEDERA_SECOND_SIGNER_KEY=302e...");
    console.log("  Then the example will call schedule.sign() and wait for execution.");

    printSection("Cancelling Pending Schedule — schedule.delete()");
    console.log("  Deleting the schedule since no second signer was provided...");

    try {
      await client.schedule.delete({ scheduleId: created.scheduleId });
      console.log("  Deleted successfully.");
    } catch (err) {
      console.log("  Delete failed (may already be expired/executed):");
      printError(err);
    }

    // Confirm deletion by querying again
    try {
      const afterDelete = await client.schedule.get(created.scheduleId);
      console.log(`  Status after delete: ${afterDelete.status}`); 
      if (afterDelete.deletionTimestamp) {
        console.log(`  Deleted at: ${formatTimestamp(afterDelete.deletionTimestamp)}`);
      }
    } catch (err) {
      if (err instanceof HieroRuntimeError && err.code === "NOT_FOUND") {
        console.log("  Schedule no longer found (removed from state).");
      } else {
        printError(err);
      }
    }
  }
}

// ── 4. Multi-sig explanation ──────────────────────────────────────────────────

printSection("How Multi-Party Signing Works");

console.log(`
  Real-world multi-sig scheduled transfer scenario:

  1. Party A creates the schedule (pays the ScheduleCreateTransaction fee):
       client.schedule.createTransfer({
         fromAccountId: "0.0.PARTY_A",   // Party A's account
         toAccountId:   "0.0.RECIPIENT",
         amountTinybar: "100000000",
       })
       → status: "pendingSignatures"

  2. Party A signs with their key:
       client.schedule.sign({ scheduleId, signerPrivateKey: partyAKey })

  3. Party B signs independently from their own system:
       client.schedule.sign({ scheduleId, signerPrivateKey: partyBKey })
       → if threshold is met → status: "executed"

  4. Either party polls until execution:
       const result = await client.schedule.wait(scheduleId)
       // result.finalized.receipt.status === "SUCCESS"

  Key property: Party B never gives their private key to Party A.
  The schedule lives on-ledger and accumulates signatures trustlessly.
`);

console.log("Done.");