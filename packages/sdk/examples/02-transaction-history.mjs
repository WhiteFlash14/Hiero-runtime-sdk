/**
 * Example 02 — Paginated Transaction History
 *
 * Demonstrates:
 *   - mirror.transactions.list()  — manual cursor-based pagination
 *   - mirror.transactions.pages() — async generator that walks all pages
 *   - Counting/summarising transaction types across pages
 *   - performance.now() timing
 *
 *   node packages/sdk/examples/02-transaction-history.mjs
 */

import { createClient } from "../dist/index.js";
import { printSection, formatTimestamp, summarizeTxTypes } from "./_utils.mjs";

const ACCOUNT_ID = "0.0.98"; 
const PAGE_SIZE = 10;

const client = await createClient({ network: "testnet" });

// ── 1. Manual cursor pagination ───────────────────────────────────────────────
// list() returns { items, nextCursor }. Pass nextCursor back to fetch the
// next page. Stop when nextCursor is undefined.

printSection("Manual Cursor Pagination — mirror.transactions.list()");

const page1 = await client.mirror.transactions.list(ACCOUNT_ID, {
  limit: PAGE_SIZE,
});

console.log(`  Page 1:  ${page1.items.length} transactions`);
console.log(`  Types:   ${summarizeTxTypes(page1.items)}`);
console.log(
  `  Oldest:  ${formatTimestamp(page1.items.at(-1)?.consensusTimestamp)}`,
);
console.log(
  `  Next cursor: ${page1.nextCursor ? page1.nextCursor.slice(0, 60) + "…" : "(none)"}`,
);

if (page1.nextCursor) {
  const page2 = await client.mirror.transactions.list(ACCOUNT_ID, {
    cursor: page1.nextCursor,
  });

  console.log();
  console.log(`  Page 2:  ${page2.items.length} transactions`);
  console.log(`  Types:   ${summarizeTxTypes(page2.items)}`);
  console.log(
    `  Oldest:  ${formatTimestamp(page2.items.at(-1)?.consensusTimestamp)}`,
  );
  console.log(
    `  Has more: ${page2.nextCursor ? "yes" : "no"}`,
  );
}

// ── 2. Async generator — auto-walk all pages ──────────────────────────────────
// pages() returns an AsyncGenerator that handles cursor management internally.
// Each iteration yields one page's worth of MirrorTransactionRecord[].
// This is the pattern to use when you want all records without managing cursors.

printSection("Async Generator — mirror.transactions.pages() (first 4 pages)");

const t0 = performance.now();
let pageNum = 0;
let totalTxs = 0;
/** @type {Map<string, number>} */
const typeTotals = new Map();

for await (const page of client.mirror.transactions.pages(ACCOUNT_ID, {
  limit: PAGE_SIZE,
})) {
  pageNum++;
  totalTxs += page.length;

  // Accumulate type counts
  for (const tx of page) {
    const name = tx.name ?? "(unknown)";
    typeTotals.set(name, (typeTotals.get(name) ?? 0) + 1);
  }

  const types = summarizeTxTypes(page);
  const oldest = formatTimestamp(page.at(-1)?.consensusTimestamp);
  console.log(`  Page ${pageNum}:  ${page.length} txs | ${types}`);
  console.log(`           oldest: ${oldest}`);

  if (pageNum >= 4) {
    console.log("  (stopping after 4 pages — remove the break to fetch all)");
    break;
  }
}

const elapsed = (performance.now() - t0).toFixed(0);

// ── 3. Summary ────────────────────────────────────────────────────────────────

printSection("Summary");

console.log(`  Pages fetched:  ${pageNum}`);
console.log(`  Total txs:      ${totalTxs}`);
console.log(`  Time:           ${elapsed}ms`);
console.log();
console.log("  Transaction type distribution:");
for (const [name, count] of [...typeTotals.entries()].sort(
  (a, b) => b[1] - a[1],
)) {
  const bar = "█".repeat(Math.ceil((count / totalTxs) * 20));
  console.log(`    ${name.padEnd(30)} ${String(count).padStart(3)}  ${bar}`);
}

console.log("\nDone.");
