/**
 * Benchmark — Mirror Node Query Performance
 *
 * Measures real latency of Mirror Node queries via the Hiero Runtime SDK
 * and compares against raw fetch() to quantify native addon overhead.
 *
 * What this benchmark answers:
 *   1. What is P50/P95/P99 latency for a single Mirror account query?
 *   2. How much faster is parallel vs sequential for N queries?
 *   3. How long does paginating N pages take?
 *   4. What is the overhead of the Rust/NAPI layer vs raw fetch()?
 *
 *   node packages/sdk/examples/benchmark.mjs
 *
 * Override the network (default: testnet):
 *   HEDERA_NETWORK=mainnet node packages/sdk/examples/benchmark.mjs
 */

import { createClient } from "../dist/index.js";
import { printSection, percentiles } from "./_utils.mjs";

const network = process.env.HEDERA_NETWORK ?? "testnet";
const MIRROR_BASES = {
  testnet:    "https://testnet.mirrornode.hedera.com",
  mainnet:    "https://mainnet.mirrornode.hedera.com",
  previewnet: "https://previewnet.mirrornode.hedera.com",
};

const ACCOUNT_ID = "0.0.98"; // Hedera treasury 
const MIRROR_BASE = MIRROR_BASES[network] ?? MIRROR_BASES.testnet;
const WARMUP = 3; 
const RUNS = 20; 
const PARALLEL_N = 8; 
const PAGE_COUNT = 5;


const PARALLEL_ACCOUNTS = [
  "0.0.2", "0.0.3", "0.0.4", "0.0.5",
  "0.0.6", "0.0.7", "0.0.8", "0.0.98",
];

const client = await createClient({ network: "testnet" });

// ── 1. Warm up the connection pool ────────────────────────────────────────────
// reqwest (the Rust HTTP client) reuses connections. The first request pays
// TCP + TLS handshake overhead. Discard warm-up runs to get steady-state numbers.

printSection("Warming up...");
for (let i = 0; i < WARMUP; i++) {
  await client.mirror.accounts.get(ACCOUNT_ID);
}
console.log(`  ${WARMUP} warm-up requests done`);

// ── 2. Single account query latency ──────────────────────────────────────────

printSection(`Single Account Query — ${RUNS} runs (account ${ACCOUNT_ID})`);

const singleTimes = [];
for (let i = 0; i < RUNS; i++) {
  const t = performance.now();
  await client.mirror.accounts.get(ACCOUNT_ID);
  singleTimes.push(performance.now() - t);
}

const s = percentiles(singleTimes);
console.log(`  Min:  ${s.min.toFixed(1)}ms`);
console.log(`  P50:  ${s.p50.toFixed(1)}ms`);
console.log(`  P95:  ${s.p95.toFixed(1)}ms`);
console.log(`  P99:  ${s.p99.toFixed(1)}ms`);
console.log(`  Max:  ${s.max.toFixed(1)}ms`);
console.log(`  Avg:  ${s.avg.toFixed(1)}ms`);

// ── 3. Sequential vs parallel ─────────────────────────────────────────────────

printSection(`Sequential vs Parallel — ${PARALLEL_N} accounts`);

// Sequential
const t0 = performance.now();
for (const id of PARALLEL_ACCOUNTS) {
  await client.mirror.accounts.get(id);
}
const seqTime = performance.now() - t0;

// Parallel
const t1 = performance.now();
await Promise.all(PARALLEL_ACCOUNTS.map((id) => client.mirror.accounts.get(id)));
const parTime = performance.now() - t1;

const speedup = seqTime / parTime;
console.log(`  Sequential:  ${seqTime.toFixed(0)}ms  (${PARALLEL_N} requests, one at a time)`);
console.log(`  Parallel:    ${parTime.toFixed(0)}ms  (${PARALLEL_N} requests, all at once)`);
console.log(`  Speedup:     ${speedup.toFixed(1)}×`);

// ── 4. Pagination throughput ──────────────────────────────────────────────────

printSection(`Pagination — ${PAGE_COUNT} pages × 25 transactions`);

const t2 = performance.now();
let txCount = 0;
let pageNum = 0;

for await (const page of client.mirror.transactions.pages(ACCOUNT_ID, { limit: 25 })) {
  txCount += page.length;
  pageNum++;
  if (pageNum >= PAGE_COUNT) break;
}

const pagTime = performance.now() - t2;
console.log(`  ${pageNum} pages fetched, ${txCount} transactions total`);
console.log(`  Total time:  ${pagTime.toFixed(0)}ms`);
console.log(`  Per page:    ${(pagTime / pageNum).toFixed(0)}ms avg`);
console.log(`  Per tx:      ${(pagTime / txCount).toFixed(1)}ms avg`);

// ── 5. SDK overhead vs raw fetch ──────────────────────────────────────────────
// Measures the extra latency introduced by the Rust NAPI layer + JSON parsing
// compared to the minimal overhead of calling the Mirror Node with fetch().
// A small overhead here is expected and acceptable — it's the price of the
// structured error handling, retry logic, and DTO validation the SDK provides.

printSection(`SDK Overhead vs Raw fetch() — ${RUNS} runs`);

const rawUrl = `${MIRROR_BASE}/api/v1/accounts/${ACCOUNT_ID}`;

// Warm up fetch too
for (let i = 0; i < WARMUP; i++) {
  const r = await fetch(rawUrl);
  await r.json();
}

// Measure raw fetch
const fetchTimes = [];
for (let i = 0; i < RUNS; i++) {
  const t = performance.now();
  const r = await fetch(rawUrl);
  await r.json();
  fetchTimes.push(performance.now() - t);
}

// Measure SDK
const sdkTimes = [];
for (let i = 0; i < RUNS; i++) {
  const t = performance.now();
  await client.mirror.accounts.get(ACCOUNT_ID);
  sdkTimes.push(performance.now() - t);
}

const f = percentiles(fetchTimes);
const k = percentiles(sdkTimes);
const overheadPct = (((k.avg - f.avg) / f.avg) * 100).toFixed(1);
const overheadAbs = (k.avg - f.avg).toFixed(1);

console.log(`\n  ${"".padEnd(12)} ${"fetch".padStart(8)} ${"SDK".padStart(8)}`);
console.log(`  ${"─".repeat(30)}`);
console.log(`  ${"P50".padEnd(12)} ${f.p50.toFixed(1).padStart(7)}ms ${k.p50.toFixed(1).padStart(7)}ms`);
console.log(`  ${"P95".padEnd(12)} ${f.p95.toFixed(1).padStart(7)}ms ${k.p95.toFixed(1).padStart(7)}ms`);
console.log(`  ${"Avg".padEnd(12)} ${f.avg.toFixed(1).padStart(7)}ms ${k.avg.toFixed(1).padStart(7)}ms`);
console.log();

if (Math.abs(Number(overheadPct)) < 10) {
  console.log(
    `  SDK overhead: +${overheadAbs}ms avg (+${overheadPct}%) — negligible`,
  );
  console.log(
    "  The Rust NAPI layer adds no meaningful latency on top of raw fetch.",
  );
} else {
  console.log(`  SDK overhead: +${overheadAbs}ms avg (+${overheadPct}%)`);
}

console.log(`\n  Note: both fetch and SDK use the same underlying HTTP connection pool`);
console.log(`  (reqwest in Rust; undici in Node). Network RTT dominates total latency.`);

// ── 6. Summary ────────────────────────────────────────────────────────────────

printSection("Summary");

console.log(`  Mirror Node (testnet) — single query P50: ${s.p50.toFixed(0)}ms`);
console.log(`  Parallel speedup (${PARALLEL_N} queries):         ${speedup.toFixed(1)}×`);
console.log(`  Pagination throughput:              ${(txCount / (pagTime / 1000)).toFixed(0)} tx/sec`);
console.log(`  Native addon overhead vs fetch:     +${overheadAbs}ms avg`);

console.log("\nDone.");