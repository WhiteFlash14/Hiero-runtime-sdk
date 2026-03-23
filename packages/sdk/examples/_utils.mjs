import { HieroRuntimeError } from "../dist/index.js";

export function tinybarsToHbar(tinybarStr) {
  const tinybar = BigInt(tinybarStr);
  const whole = tinybar / 100_000_000n;
  const remainder = tinybar % 100_000_000n;
  const fracStr = remainder.toString().padStart(8, "0");
  const wholeFormatted = whole.toLocaleString("en-US");
  return `${wholeFormatted}.${fracStr} HBAR`;
}
/**
 * Format a Hedera consensus timestamp ("seconds.nanoseconds") as ISO 8601.
 *
 * @param {string | undefined} ts - e.g. "1701234567.123456789"
 * @returns {string}
 */
export function formatTimestamp(ts) {
  if (!ts) return "(unknown)";
  const [secs] = ts.split(".");
  return new Date(Number(secs) * 1000).toISOString();
}

/**
 * Print a section header to make console output easy to scan.
 *
 * @param {string} title
 */
export function printSection(title) {
  console.log(`\n${"─".repeat(60)}`);
  console.log(`  ${title}`);
  console.log("─".repeat(60));
}

/**
 * Pretty-print a caught error, showing extra fields for HieroRuntimeError.
 *
 * @param {unknown} err
 */
export function printError(err) {
  if (err instanceof HieroRuntimeError) {
    console.log(`  Error code:  ${err.code}`);
    console.log(`  Retryable:   ${err.retryable}`);
    console.log(`  Message:     ${err.message}`);
    if (err.details !== undefined && err.details !== null) {
      console.log(`  Details:     ${JSON.stringify(err.details)}`);
    }
  } else {
    console.log(`  (non-runtime error) ${String(err)}`);
  }
}

/**
 * Collect percentile stats from an array of millisecond timings.
 *
 * @param {number[]} times
 * @returns {{ min: number, p50: number, p95: number, p99: number, max: number, avg: number }}
 */
export function percentiles(times) {
  const sorted = [...times].sort((a, b) => a - b);
  const n = sorted.length;
  const p = (pct) => sorted[Math.floor((pct / 100) * (n - 1))];
  const avg = times.reduce((s, v) => s + v, 0) / n;
  return {
    min: sorted[0],
    p50: p(50),
    p95: p(95),
    p99: p(99),
    max: sorted[n - 1],
    avg,
  };
}

/**
 * Count occurrences of each `name` field in an array of transaction records.
 *
 * @param {Array<{ name?: string }>} records
 * @returns {string} e.g. "CRYPTOTRANSFER x22, TOKENMINT x3"
 */
export function summarizeTxTypes(records) {
  /** @type {Map<string, number>} */
  const counts = new Map();
  for (const r of records) {
    const key = r.name ?? "(unknown)";
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1])
    .map(([name, n]) => `${name} ×${n}`)
    .join(", ");
}
