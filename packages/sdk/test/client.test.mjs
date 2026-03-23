/**
 * Integration tests for @hiero-runtime/sdk.
 */

import { before, describe, it } from "node:test";
import assert from "node:assert/strict";


import { createClient, HieroRuntimeError } from "../dist/index.js";

// ── helpers ──────────────────────────────────────────────────────────────────


async function assertRuntimeError(fn, expectedCode) {
  let threw = false;
  try {
    await fn();
  } catch (err) {
    threw = true;
    assert.ok(
      err instanceof HieroRuntimeError,
      `expected HieroRuntimeError, got ${err?.constructor?.name}: ${err?.message}`,
    );
    assert.equal(
      err.code,
      expectedCode,
      `expected code ${expectedCode}, got ${err.code}`,
    );
  }
  assert.ok(threw, `expected an error with code ${expectedCode} to be thrown`);
}

const networkTest = process.env.HIERO_TEST_NETWORK === "testnet";

// ── Config validation  ────────────────────────

describe("createClient – config validation", () => {
  it("rejects empty operator.accountId", async () => {
    await assertRuntimeError(
      () =>
        createClient({
          network: "testnet",
          operator: { accountId: "  ", privateKey: "dummy" },
        }),
      "INVALID_CONFIG",
    );
  });

  it("rejects empty operator.privateKey", async () => {
    await assertRuntimeError(
      () =>
        createClient({
          network: "testnet",
          operator: { accountId: "0.0.1", privateKey: "" },
        }),
      "INVALID_CONFIG",
    );
  });

  it("rejects zero retry.maxAttempts", async () => {
    await assertRuntimeError(
      () => createClient({ network: "testnet", retry: { maxAttempts: 0 } }),
      "INVALID_CONFIG",
    );
  });

  it("rejects initialDelayMs > maxDelayMs", async () => {
    await assertRuntimeError(
      () =>
        createClient({
          network: "testnet",
          retry: { initialDelayMs: 5000, maxDelayMs: 100 },
        }),
      "INVALID_CONFIG",
    );
  });

  it("rejects zero finality.receiptTimeoutMs", async () => {
    await assertRuntimeError(
      () =>
        createClient({
          network: "testnet",
          finality: { receiptTimeoutMs: 0 },
        }),
      "INVALID_CONFIG",
    );
  });

  it("rejects custom network without consensusNodes", async () => {
    await assertRuntimeError(
      () => createClient({ network: "custom", mirror: { baseUrl: "https://mirror.example.com" } }),
      "INVALID_CONFIG",
    );
  });

  it("rejects custom network with empty consensusNodes", async () => {
    await assertRuntimeError(
      () =>
        createClient({
          network: "custom",
          mirror: { baseUrl: "https://mirror.example.com" },
          consensusNodes: [],
        }),
      "INVALID_CONFIG",
    );
  });

  it("rejects custom network without mirror.baseUrl", async () => {
    await assertRuntimeError(
      () =>
        createClient({
          network: "custom",
          consensusNodes: [{ url: "34.94.106.61:50211", accountId: "0.0.3" }],
        }),
      "INVALID_CONFIG",
    );
  });
});

// ── Native bootstrap ─────────────────

describe("createClient – native bootstrap", () => {
  it("constructs a client for testnet without aborting", async () => {
    const client = await createClient({ network: "testnet" });
    assert.ok(client, "expected a client instance");
  });

  it("exposes the correct config after creation", async () => {
    const client = await createClient({ network: "testnet" });
    assert.equal(client.config.network.kind, "testnet");
    assert.ok(
      client.config.network.mirrorBaseUrl.includes("testnet"),
      "mirrorBaseUrl should contain 'testnet'",
    );
  });

  it("returns addon metadata via getNativeMetadata()", async () => {
    const client = await createClient({ network: "testnet" });
    const meta = client.getNativeMetadata();
    assert.ok(meta, "expected metadata object");
    assert.equal(
      meta.packageName,
      "@hiero-runtime/bindings-node",
      "packageName mismatch",
    );
    assert.ok(
      typeof meta.version === "string" && meta.version.length > 0,
      "expected non-empty version string",
    );
  });

  it("exposes all schedule / mirror / finality / submit namespaces", async () => {
    const client = await createClient({ network: "testnet" });
    assert.ok(typeof client.mirror?.accounts?.get === "function");
    assert.ok(typeof client.mirror?.transactions?.get === "function");
    assert.ok(typeof client.mirror?.transactions?.list === "function");
    assert.ok(typeof client.mirror?.transactions?.pages === "function");
    assert.ok(typeof client.mirror?.contracts?.getResult === "function");
    assert.ok(typeof client.submit?.hbarTransfer === "function");
    assert.ok(typeof client.finality?.waitForReceipt === "function");
    assert.ok(typeof client.finality?.wait === "function");
    assert.ok(typeof client.schedule?.createTransfer === "function");
    assert.ok(typeof client.schedule?.sign === "function");
    assert.ok(typeof client.schedule?.get === "function");
    assert.ok(typeof client.schedule?.delete === "function");
    assert.ok(typeof client.schedule?.wait === "function");
  });

  it("mirror.transactions.pages returns an async generator", async () => {
    const client = await createClient({ network: "testnet" });
    const gen = client.mirror.transactions.pages("0.0.98");
    assert.ok(typeof gen.next === "function", "expected next() method");
    assert.ok(
      typeof gen[Symbol.asyncIterator] === "function",
      "expected async iterator protocol",
    );
  });

  it("attach() builds a handle with the correct transactionId", async () => {
    const client = await createClient({ network: "testnet" });
    const handle = client.attach("0.0.1001@1700000000.000000000");
    assert.equal(handle.transactionId, "0.0.1001@1700000000.000000000");
    assert.ok(typeof handle.waitForReceipt === "function");
    assert.ok(typeof handle.waitForFinality === "function");
  });
});

// ── Input-validation guards ───────────────────

describe("client method – input validation", () => {
  let client;

  before(async () => {
    client = await createClient({ network: "testnet" });
  });

  it("mirror.accounts.get rejects empty id", async () => {
    await assertRuntimeError(() => client.mirror.accounts.get(""), "INVALID_CONFIG");
  });

  it("mirror.transactions.get rejects empty transactionId", async () => {
    await assertRuntimeError(
      () => client.mirror.transactions.get("   "),
      "INVALID_CONFIG",
    );
  });

  it("mirror.contracts.getResult rejects empty hash", async () => {
    await assertRuntimeError(
      () => client.mirror.contracts.getResult(""),
      "INVALID_CONFIG",
    );
  });

  it("submit.hbarTransfer rejects empty fromAccountId", async () => {
    await assertRuntimeError(
      () =>
        client.submit.hbarTransfer({
          fromAccountId: "",
          toAccountId: "0.0.2",
          amountTinybar: "1",
        }),
      "INVALID_CONFIG",
    );
  });

  it("submit.hbarTransfer rejects zero amountTinybar", async () => {
    await assertRuntimeError(
      () =>
        client.submit.hbarTransfer({
          fromAccountId: "0.0.1",
          toAccountId: "0.0.2",
          amountTinybar: "0",
        }),
      "INVALID_CONFIG",
    );
  });

  it("submit.hbarTransfer rejects negative amountTinybar", async () => {
    await assertRuntimeError(
      () =>
        client.submit.hbarTransfer({
          fromAccountId: "0.0.1",
          toAccountId: "0.0.2",
          amountTinybar: "-10",
        }),
      "INVALID_CONFIG",
    );
  });

  it("schedule.get rejects empty scheduleId", async () => {
    await assertRuntimeError(() => client.schedule.get(""), "INVALID_CONFIG");
  });

  it("schedule.delete rejects empty scheduleId", async () => {
    await assertRuntimeError(
      () => client.schedule.delete({ scheduleId: "" }),
      "INVALID_CONFIG",
    );
  });

  it("schedule.wait rejects empty scheduleId", async () => {
    await assertRuntimeError(() => client.schedule.wait(""), "INVALID_CONFIG");
  });

  it("schedule.sign rejects empty scheduleId", async () => {
    await assertRuntimeError(
      () =>
        client.schedule.sign({
          scheduleId: "",
          signerPrivateKey: "dummy",
        }),
      "INVALID_CONFIG",
    );
  });

  it("schedule.sign rejects empty signerPrivateKey", async () => {
    await assertRuntimeError(
      () =>
        client.schedule.sign({
          scheduleId: "0.0.1",
          signerPrivateKey: "  ",
        }),
      "INVALID_CONFIG",
    );
  });

  it("schedule.createTransfer rejects zero amountTinybar", async () => {
    await assertRuntimeError(
      () =>
        client.schedule.createTransfer({
          fromAccountId: "0.0.1",
          toAccountId: "0.0.2",
          amountTinybar: "0",
        }),
      "INVALID_CONFIG",
    );
  });

  it("mirror.transactions.list rejects empty accountId", async () => {
    await assertRuntimeError(
      () => client.mirror.transactions.list(""),
      "INVALID_CONFIG",
    );
  });

  it("attach() rejects empty transactionId", () => {
    assert.throws(
      () => client.attach(""),
      (err) => err instanceof HieroRuntimeError && err.code === "INVALID_CONFIG",
    );
  });
});

// ── Mirror network smoke tests  ─────────────────────────────

describe("mirror network – testnet smoke", { skip: !networkTest }, () => {
  let client;

  before(async () => {
    client = await createClient({ network: "testnet" });
  });

  it("mirror.accounts.get returns account info for 0.0.98", async () => {
    const account = await client.mirror.accounts.get("0.0.98");
    assert.ok(account.account, "expected account field");
    assert.ok(typeof account.balance === "string", "expected balance as string");
    assert.equal(account.deleted, false);
  });

  it("mirror.accounts.get throws NOT_FOUND for a nonexistent account", async () => {
    await assertRuntimeError(
      () => client.mirror.accounts.get("0.0.99999999999"),
      "NOT_FOUND",
    );
  });
});
