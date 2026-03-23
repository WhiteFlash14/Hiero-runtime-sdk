# Hiero Runtime SDK — Examples

Runnable examples demonstrating every feature of `@hiero-runtime/sdk` against the live Hedera testnet.

## Setup

```bash
# From the repo root — builds the Rust native addon and compiles TypeScript
pnpm install && pnpm build
```

That is the only setup required for the no-credential examples.

---

## Examples

| File | What it shows | Credentials? |
|------|--------------|:------------:|
| [01-account-lookup.mjs](01-account-lookup.mjs) | Mirror Node account query, balance in HBAR, error handling | No |
| [02-transaction-history.mjs](02-transaction-history.mjs) | Manual cursor pagination + `pages()` async generator | No |
| [03-transaction-lookup.mjs](03-transaction-lookup.mjs) | Single transaction lookup, `?scheduled` / `?nonce` ID formats | No |
| [04-error-handling.mjs](04-error-handling.mjs) | All error codes, custom retry config, config validation | No |
| [05-hbar-transfer.mjs](05-hbar-transfer.mjs) | HBAR transfer, two-phase finality, `attach()` pattern | **Yes** |
| [06-scheduled-transfer.mjs](06-scheduled-transfer.mjs) | Full schedule lifecycle: create → sign → wait → execute | **Yes** |
| [07-all-networks.mjs](07-all-networks.mjs) | testnet / mainnet / previewnet / custom network config | No |
| [benchmark.mjs](benchmark.mjs) | Latency percentiles, parallel vs sequential, overhead vs `fetch` | No |

---

## Running without credentials (immediate)

```bash
node packages/sdk/examples/01-account-lookup.mjs
node packages/sdk/examples/02-transaction-history.mjs
node packages/sdk/examples/03-transaction-lookup.mjs
node packages/sdk/examples/04-error-handling.mjs
node packages/sdk/examples/07-all-networks.mjs
node packages/sdk/examples/benchmark.mjs
```

Each of these hits the live Hedera testnet Mirror Node (public, no authentication) and produces real output in seconds.

---

## Running with credentials

Examples 05 and 06 submit real transactions to testnet. You need a testnet account — free from [portal.hedera.com](https://portal.hedera.com).

```bash
export HEDERA_OPERATOR_ID=0.0.12345
export HEDERA_OPERATOR_KEY=302e020100300506032b6570...
export HEDERA_RECEIVER_ID=0.0.67890        # optional — defaults to 0.0.98

# HBAR transfer with two-phase finality
node packages/sdk/examples/05-hbar-transfer.mjs

# Scheduled transfer lifecycle
node packages/sdk/examples/06-scheduled-transfer.mjs

# Optional — enables multi-party signing path in example 06
export HEDERA_SECOND_SIGNER_KEY=302e...
node packages/sdk/examples/06-scheduled-transfer.mjs
```

If the environment variables are not set, the examples print instructions and exit cleanly — they never crash.

---

## Key concepts demonstrated

### Two-phase finality (example 05)
`finality.wait()` combines two polling loops that existing Hedera SDKs require you to write manually:
1. **Receipt phase** — polls consensus nodes until the transaction is included in a block (~2–5s)
2. **Mirror phase** — waits until the Mirror Node makes the record queryable (~3–8s after receipt)

The finalized result contains both the consensus receipt and the Mirror Node record.

### Pagination (example 02)
Mirror Node list endpoints return pages of results with cursor-based navigation.

```javascript
// Manual — you control the cursor
const page1 = await client.mirror.transactions.list("0.0.98", { limit: 25 });
const page2 = await client.mirror.transactions.list("0.0.98", { cursor: page1.nextCursor });

// Automatic — async generator walks all pages
for await (const page of client.mirror.transactions.pages("0.0.98")) {
  // page is MirrorTransactionRecord[]
}
```

### Schedule lifecycle (example 06)
Scheduled transactions allow multiple parties to agree on a future on-ledger action without any party holding another's private key. The schedule lives on-chain and accumulates signatures until the threshold is met.

```javascript
// Party A creates the schedule
const { scheduleId } = await client.schedule.createTransfer({ ... });

// Party B signs independently
await client.schedule.sign({ scheduleId, signerPrivateKey: partyBKey });

// Either party waits for execution
const { finalized } = await client.schedule.wait(scheduleId);
```

### Structured errors (example 04)
Every error is a `HieroRuntimeError` with a stable machine-readable `.code`, a `.retryable` flag, and optional `.details`:

```javascript
try {
  await client.mirror.accounts.get("0.0.99999999999");
} catch (err) {
  if (err instanceof HieroRuntimeError) {
    err.code      // "NOT_FOUND"
    err.retryable // false
    err.message   // human-readable description
    err.details   // { path: "/api/v1/accounts/..." }
  }
}
```
