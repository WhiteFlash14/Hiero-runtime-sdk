import { createClient, HieroRuntimeError } from "../dist/index.js";
import { tinybarsToHbar, printSection, printError } from "./_utils.mjs";

// ── 1. Create clients for all named networks ──────────────────────────────────
// createClient() is deliberately cheap — it only initialises the Mirror HTTP
// client, never the Hiero SDK consensus client. This makes it safe to create
// multiple clients at startup without paying any network setup cost.

printSection("Creating Clients for All Networks");

const [testnetClient, mainnetClient, previewnetClient] = await Promise.all([
  createClient({ network: "testnet" }),
  createClient({ network: "mainnet" }),
  createClient({ network: "previewnet" }),
]);

console.log("  testnet    → " + testnetClient.config.network.mirrorBaseUrl);
console.log("  mainnet    → " + mainnetClient.config.network.mirrorBaseUrl);
console.log("  previewnet → " + previewnetClient.config.network.mirrorBaseUrl);

// ── 2. Query 0.0.98 on all networks in parallel ───────────────────────────────
// Account 0.0.98 is the Hedera treasury on every network. Balances differ
// because each network is independent — testnet is reset periodically.

printSection("Querying 0.0.98 (Treasury) on All Networks — in Parallel");

console.log("  Dispatching three requests simultaneously...\n");
const t0 = performance.now();

const results = await Promise.allSettled([
  testnetClient.mirror.accounts.get("0.0.98"),
  mainnetClient.mirror.accounts.get("0.0.98"),
  previewnetClient.mirror.accounts.get("0.0.98"),
]);

const elapsed = (performance.now() - t0).toFixed(0);
const networks = ["testnet", "mainnet", "previewnet"];

for (let i = 0; i < results.length; i++) {
  const r = results[i];
  const label = networks[i].padEnd(10);
  if (r.status === "fulfilled") {
    const { account, balance, deleted } = r.value;
    console.log(`  ${label}  account=${account}  balance=${tinybarsToHbar(balance)}  deleted=${deleted}`);
  } else {
    const code =
      r.reason instanceof HieroRuntimeError ? r.reason.code : "UNKNOWN";
    console.log(`  ${label}  ✗ ${code} (${r.reason?.message?.slice(0, 60)})`);
  }
}

console.log(`\n  All three queries completed in ${elapsed}ms (parallel, not sequential)`);

// ── 3. Inspect client.config ──────────────────────────────────────────────────
// client.config exposes the full resolved configuration, including all defaults
// that were filled in by createClient(). Useful for debugging or logging.

printSection("Inspecting client.config");

const cfg = testnetClient.config;
console.log("  network:");
console.log(`    kind:          ${cfg.network.kind}`);
console.log(`    mirrorBaseUrl: ${cfg.network.mirrorBaseUrl}`);
console.log(`    consensusNodes:${cfg.network.consensusNodes ? JSON.stringify(cfg.network.consensusNodes) : " (auto-discovered by SDK)"}`);
console.log("  retry:");
console.log(`    maxAttempts:   ${cfg.retry.maxAttempts}`);
console.log(`    initialDelay:  ${cfg.retry.initialDelayMs}ms`);
console.log(`    maxDelay:      ${cfg.retry.maxDelayMs}ms`);
console.log(`    jitter:        ${cfg.retry.jitter}`);
console.log("  finality:");
console.log(`    receiptTimeout:${cfg.finality.receiptTimeoutMs}ms`);
console.log(`    mirrorTimeout: ${cfg.finality.mirrorTimeoutMs}ms`);
console.log(`    pollInterval:  ${cfg.finality.pollIntervalMs}ms`);
console.log("  operator:        (none — read-only client)");

// ── 4. Custom network configuration ──────────────────────────────────────────
// Custom networks require both an explicit Mirror base URL and a list of
// consensus node endpoints. This is used for private Hedera nodes such as
// the Hedera Local Node (https://github.com/hashgraph/hedera-local-node).

printSection("Custom Network Config (Local Node)");

console.log("  A custom network requires explicit mirror + consensus endpoints.");
console.log("  This is used for private deployments or local development nodes.\n");

// Show how the config looks — we don't actually connect since there's no
// local node running, but the client creation itself succeeds.
let customClient;
try {
  customClient = await createClient({
    network: "custom",
    mirror: { baseUrl: "http://localhost:5551" },
    consensusNodes: [
      { url: "localhost:50211", accountId: "0.0.3" },
    ],
  });

  const customCfg = customClient.config;
  console.log("  Custom client created:");
  console.log(`    mirrorBaseUrl:  ${customCfg.network.mirrorBaseUrl}`);
  console.log(`    consensusNodes: ${JSON.stringify(customCfg.network.consensusNodes)}`);
  console.log("\n  (A query against localhost would fail unless a local node is running)");
} catch (err) {
  if (err instanceof HieroRuntimeError) {
    console.log(`  Client creation failed: ${err.code} — ${err.message}`);
  }
}

console.log();
console.log("  To run a local Hedera node:");
console.log("    npx @hashgraph/hedera-local start");
console.log("  Then set network: 'custom' with localhost endpoints.");

// ── 5. Parallel account queries on testnet ────────────────────────────────────
// Demonstrate that multiple accounts can be fetched concurrently.

printSection("Parallel Account Queries on Testnet");

const SYSTEM_ACCOUNTS = ["0.0.2", "0.0.98", "0.0.800", "0.0.50"];

console.log(`  Fetching ${SYSTEM_ACCOUNTS.length} system accounts in parallel...\n`);
const t1 = performance.now();

const accountResults = await Promise.allSettled(
  SYSTEM_ACCOUNTS.map((id) => testnetClient.mirror.accounts.get(id)),
);

const parallelTime = (performance.now() - t1).toFixed(0);

for (let i = 0; i < accountResults.length; i++) {
  const r = accountResults[i];
  const id = SYSTEM_ACCOUNTS[i].padEnd(10);
  if (r.status === "fulfilled") {
    console.log(`  ${id}  balance=${tinybarsToHbar(r.value.balance)}`);
  } else {
    const code =
      r.reason instanceof HieroRuntimeError ? r.reason.code : "ERROR";
    console.log(`  ${id}  ✗ ${code}`);
  }
}

console.log(`\n  Completed in ${parallelTime}ms`);

console.log("\nDone.");