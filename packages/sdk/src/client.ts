import { NativeRuntime, getAddonMetadata } from "@hiero-runtime/bindings-node";

import { HieroRuntimeError, invalidConfig } from "./errors.js";
import type {
  AttachedTransactionHandle,
  ContractResultView,
  CreateClientOptions,
  CreateScheduledTransferInput,
  CreatedSchedule,
  DeleteScheduleInput,
  FinalityPolicy,
  FinalizedTransaction,
  HbarTransferInput,
  ListTransactionsOptions,
  MirrorAccountView,
  MirrorTransactionRecord,
  PageResult,
  ReceiptResult,
  RetryPolicy,
  RuntimeConfig,
  ScheduleExecution,
  ScheduleInfoView,
  SignScheduleInput,
  SubmittedTransaction,
  TransactionLookup,
} from "./types.js";

const DEFAULT_RETRY: RetryPolicy = {
  maxAttempts: 8,
  initialDelayMs: 250,
  maxDelayMs: 3000,
  jitter: true,
};

const DEFAULT_FINALITY: FinalityPolicy = {
  receiptTimeoutMs: 15000,
  mirrorTimeoutMs: 20000,
  pollIntervalMs: 500,
};

const DEFAULT_MIRROR_BASE_URL: Record<
  "mainnet" | "testnet" | "previewnet",
  string
> = {
  mainnet: "https://mainnet.mirrornode.hedera.com",
  testnet: "https://testnet.mirrornode.hedera.com",
  previewnet: "https://previewnet.mirrornode.hedera.com",
};

function parseJsonResult<T>(raw: string, context: string): T {
  try {
    return JSON.parse(raw) as T;
  } catch (error) {
    throw new HieroRuntimeError({
      code: "SERIALIZATION",
      message: `failed to parse native JSON result for ${context}`,
      retryable: false,
      details: {
        cause: error instanceof Error ? error.message : String(error),
        raw,
      },
    });
  }
}

function normalizeMirrorBaseUrl(options: CreateClientOptions): string {
  if (options.network === "custom") {
    const baseUrl = options.mirror?.baseUrl?.trim();
    if (!baseUrl) {
      throw invalidConfig("mirror.baseUrl is required when network is custom", {
        network: options.network,
      });
    }

    return baseUrl;
  }

  return (
    options.mirror?.baseUrl?.trim() || DEFAULT_MIRROR_BASE_URL[options.network]
  );
}

function normalizeRuntimeConfig(options: CreateClientOptions): RuntimeConfig {
  if (options.network === "custom" && !options.consensusNodes?.length) {
    throw invalidConfig(
      "consensusNodes is required and must not be empty when network is custom",
      { network: options.network },
    );
  }

  const runtimeConfig: RuntimeConfig = {
    network: {
      kind: options.network,
      mirrorBaseUrl: normalizeMirrorBaseUrl(options),
      consensusNodes: options.consensusNodes,
    },
    operator: options.operator,
    retry: {
      maxAttempts: options.retry?.maxAttempts ?? DEFAULT_RETRY.maxAttempts,
      initialDelayMs:
        options.retry?.initialDelayMs ?? DEFAULT_RETRY.initialDelayMs,
      maxDelayMs: options.retry?.maxDelayMs ?? DEFAULT_RETRY.maxDelayMs,
      jitter: options.retry?.jitter ?? DEFAULT_RETRY.jitter,
    },
    finality: {
      receiptTimeoutMs:
        options.finality?.receiptTimeoutMs ?? DEFAULT_FINALITY.receiptTimeoutMs,
      mirrorTimeoutMs:
        options.finality?.mirrorTimeoutMs ?? DEFAULT_FINALITY.mirrorTimeoutMs,
      pollIntervalMs:
        options.finality?.pollIntervalMs ?? DEFAULT_FINALITY.pollIntervalMs,
    },
  };

  if (runtimeConfig.operator) {
    if (!runtimeConfig.operator.accountId.trim()) {
      throw invalidConfig("operator.accountId must not be empty");
    }

    if (!runtimeConfig.operator.privateKey.trim()) {
      throw invalidConfig("operator.privateKey must not be empty");
    }
  }

  if (runtimeConfig.retry.maxAttempts <= 0) {
    throw invalidConfig("retry.maxAttempts must be greater than zero");
  }

  if (runtimeConfig.retry.initialDelayMs <= 0) {
    throw invalidConfig("retry.initialDelayMs must be greater than zero");
  }

  if (runtimeConfig.retry.maxDelayMs <= 0) {
    throw invalidConfig("retry.maxDelayMs must be greater than zero");
  }

  if (runtimeConfig.retry.initialDelayMs > runtimeConfig.retry.maxDelayMs) {
    throw invalidConfig(
      "retry.initialDelayMs must be less than or equal to retry.maxDelayMs",
    );
  }

  if (runtimeConfig.finality.receiptTimeoutMs <= 0) {
    throw invalidConfig("finality.receiptTimeoutMs must be greater than zero");
  }

  if (runtimeConfig.finality.mirrorTimeoutMs <= 0) {
    throw invalidConfig("finality.mirrorTimeoutMs must be greater than zero");
  }

  if (runtimeConfig.finality.pollIntervalMs <= 0) {
    throw invalidConfig("finality.pollIntervalMs must be greater than zero");
  }

  return runtimeConfig;
}

export class HieroRuntimeClient {
  readonly #native: NativeRuntime;
  readonly #config: RuntimeConfig;

  readonly mirror: {
    transactions: {
      get: (transactionId: string) => Promise<TransactionLookup>;
      /**
       * List transactions for an account, newest first.
       *
       * Returns a page of up to `limit` records (default 25) and an opaque
       * `nextCursor` you can pass back to fetch the next page.
       */
      list: (
        accountId: string,
        options?: ListTransactionsOptions,
      ) => Promise<PageResult<MirrorTransactionRecord>>;
      /**
       * Async generator that yields one page of records at a time, walking
       * all pages until the Mirror Node reports no more results.
       *
       * @example
       * for await (const page of client.mirror.transactions.pages("0.0.1234")) {
       *   for (const tx of page) console.log(tx.transactionId);
       * }
       */
      pages: (
        accountId: string,
        options?: Omit<ListTransactionsOptions, "cursor">,
      ) => AsyncGenerator<MirrorTransactionRecord[], void, unknown>;
    };
    accounts: {
      get: (idOrAliasOrEvmAddress: string) => Promise<MirrorAccountView>;
    };
    contracts: {
      getResult: (
        transactionIdOrHash: string,
        nonce?: number,
      ) => Promise<ContractResultView>;
    };
  };

  readonly submit: {
    hbarTransfer: (input: HbarTransferInput) => Promise<SubmittedTransaction>;
  };

  readonly finality: {
    waitForReceipt: (transactionId: string) => Promise<ReceiptResult>;
    wait: (transactionId: string) => Promise<FinalizedTransaction>;
  };

  readonly schedule: {
    createTransfer: (
      input: CreateScheduledTransferInput,
    ) => Promise<CreatedSchedule>;
    sign: (input: SignScheduleInput) => Promise<ScheduleInfoView>;
    get: (scheduleId: string) => Promise<ScheduleInfoView>;
    delete: (input: DeleteScheduleInput) => Promise<void>;
    wait: (scheduleId: string) => Promise<ScheduleExecution>;
  };

  constructor(native: NativeRuntime, config: RuntimeConfig) {
    this.#native = native;
    this.#config = config;

    this.mirror = {
      transactions: {
        get: async (transactionId: string): Promise<TransactionLookup> => {
          this.#assertNonEmpty("transactionId", transactionId);

          try {
            const raw = await this.#native.getMirrorTransaction(transactionId);
            return parseJsonResult<TransactionLookup>(
              raw,
              "mirror.transactions.get",
            );
          } catch (error) {
            throw HieroRuntimeError.fromUnknown(error);
          }
        },

        list: async (
          accountId: string,
          options: ListTransactionsOptions = {},
        ): Promise<PageResult<MirrorTransactionRecord>> => {
          this.#assertNonEmpty("accountId", accountId);

          try {
            const raw = await this.#native.listTransactionsForAccount(
              accountId,
              options.limit ?? null,
              options.cursor ?? null,
            );
            return parseJsonResult<PageResult<MirrorTransactionRecord>>(
              raw,
              "mirror.transactions.list",
            );
          } catch (error) {
            throw HieroRuntimeError.fromUnknown(error);
          }
        },

        pages: (
          accountId: string,
          options: Omit<ListTransactionsOptions, "cursor"> = {},
        ): AsyncGenerator<MirrorTransactionRecord[], void, unknown> => {
          // Capture the private native reference before entering the generator.
          // Regular async generators cannot be arrow functions, so we close
          // over `native` to preserve access to the private field.
          const native = this.#native;

          async function* gen(): AsyncGenerator<
            MirrorTransactionRecord[],
            void,
            unknown
          > {
            let cursor: string | undefined;
            while (true) {
              let raw: string;
              try {
                raw = await native.listTransactionsForAccount(
                  accountId,
                  options.limit ?? null,
                  cursor ?? null,
                );
              } catch (error) {
                throw HieroRuntimeError.fromUnknown(error);
              }
              const page = parseJsonResult<PageResult<MirrorTransactionRecord>>(
                raw,
                "mirror.transactions.pages",
              );
              yield page.items;
              if (!page.nextCursor) break;
              cursor = page.nextCursor;
            }
          }

          return gen();
        },
      },

      accounts: {
        get: async (
          idOrAliasOrEvmAddress: string,
        ): Promise<MirrorAccountView> => {
          this.#assertNonEmpty("idOrAliasOrEvmAddress", idOrAliasOrEvmAddress);

          try {
            const raw =
              await this.#native.getMirrorAccount(idOrAliasOrEvmAddress);
            return parseJsonResult<MirrorAccountView>(
              raw,
              "mirror.accounts.get",
            );
          } catch (error) {
            throw HieroRuntimeError.fromUnknown(error);
          }
        },
      },

      contracts: {
        getResult: async (
          transactionIdOrHash: string,
          nonce?: number,
        ): Promise<ContractResultView> => {
          this.#assertNonEmpty("transactionIdOrHash", transactionIdOrHash);

          try {
            const raw = await this.#native.getContractResult(
              transactionIdOrHash,
              nonce ?? null,
            );
            return parseJsonResult<ContractResultView>(
              raw,
              "mirror.contracts.getResult",
            );
          } catch (error) {
            throw HieroRuntimeError.fromUnknown(error);
          }
        },
      },
    };

    this.submit = {
      hbarTransfer: async (
        input: HbarTransferInput,
      ): Promise<SubmittedTransaction> => {
        validateHbarTransferInput(input);

        try {
          const raw = await this.#native.submitHbarTransfer(
            JSON.stringify(input),
          );
          return parseJsonResult<SubmittedTransaction>(
            raw,
            "submit.hbarTransfer",
          );
        } catch (error) {
          throw HieroRuntimeError.fromUnknown(error);
        }
      },
    };

    this.finality = {
      waitForReceipt: async (transactionId: string): Promise<ReceiptResult> => {
        this.#assertNonEmpty("transactionId", transactionId);

        try {
          const raw = await this.#native.waitForReceipt(transactionId);
          return parseJsonResult<ReceiptResult>(raw, "finality.waitForReceipt");
        } catch (error) {
          throw HieroRuntimeError.fromUnknown(error);
        }
      },

      wait: async (transactionId: string): Promise<FinalizedTransaction> => {
        this.#assertNonEmpty("transactionId", transactionId);

        try {
          const raw = await this.#native.waitForFinality(transactionId);
          return parseJsonResult<FinalizedTransaction>(raw, "finality.wait");
        } catch (error) {
          throw HieroRuntimeError.fromUnknown(error);
        }
      },
    };

    this.schedule = {
      createTransfer: async (
        input: CreateScheduledTransferInput,
      ): Promise<CreatedSchedule> => {
        validateCreateScheduledTransferInput(input);

        try {
          const raw = await this.#native.createScheduledTransfer(
            JSON.stringify(input),
          );
          return parseJsonResult<CreatedSchedule>(
            raw,
            "schedule.createTransfer",
          );
        } catch (error) {
          throw HieroRuntimeError.fromUnknown(error);
        }
      },

      sign: async (input: SignScheduleInput): Promise<ScheduleInfoView> => {
        validateSignScheduleInput(input);

        try {
          const raw = await this.#native.signSchedule(JSON.stringify(input));
          return parseJsonResult<ScheduleInfoView>(raw, "schedule.sign");
        } catch (error) {
          throw HieroRuntimeError.fromUnknown(error);
        }
      },

      get: async (scheduleId: string): Promise<ScheduleInfoView> => {
        this.#assertNonEmpty("scheduleId", scheduleId);

        try {
          const raw = await this.#native.getSchedule(scheduleId);
          return parseJsonResult<ScheduleInfoView>(raw, "schedule.get");
        } catch (error) {
          throw HieroRuntimeError.fromUnknown(error);
        }
      },

      delete: async (input: DeleteScheduleInput): Promise<void> => {
        this.#assertNonEmpty("scheduleId", input.scheduleId);

        try {
          await this.#native.deleteSchedule(input.scheduleId);
        } catch (error) {
          throw HieroRuntimeError.fromUnknown(error);
        }
      },

      wait: async (scheduleId: string): Promise<ScheduleExecution> => {
        this.#assertNonEmpty("scheduleId", scheduleId);

        try {
          const raw = await this.#native.waitForScheduleExecution(scheduleId);
          return parseJsonResult<ScheduleExecution>(raw, "schedule.wait");
        } catch (error) {
          throw HieroRuntimeError.fromUnknown(error);
        }
      },
    };
  }

  get config(): RuntimeConfig {
    return this.#config;
  }

  attach(transactionId: string): AttachedTransactionHandle {
    this.#assertNonEmpty("transactionId", transactionId);

    return {
      transactionId,
      waitForReceipt: () => this.finality.waitForReceipt(transactionId),
      waitForFinality: () => this.finality.wait(transactionId),
    };
  }

  getNativeMetadata(): unknown {
    return parseJsonResult(getAddonMetadata(), "getAddonMetadata");
  }

  #assertNonEmpty(name: string, value: string): void {
    if (!value.trim()) {
      throw invalidConfig(`${name} must not be empty`);
    }
  }
}

function validateTinybarString(value: string, fieldName: string): void {
  if (!value.trim()) {
    throw invalidConfig(`${fieldName} must not be empty`);
  }
  let parsed: bigint;
  try {
    parsed = BigInt(value.trim());
  } catch {
    throw invalidConfig(`${fieldName} must be a valid positive integer string`);
  }
  if (parsed <= 0n) {
    throw invalidConfig(`${fieldName} must be a positive integer`);
  }
}

function validateHbarTransferInput(input: HbarTransferInput): void {
  if (!input.fromAccountId.trim()) {
    throw invalidConfig("fromAccountId must not be empty");
  }

  if (!input.toAccountId.trim()) {
    throw invalidConfig("toAccountId must not be empty");
  }

  validateTinybarString(input.amountTinybar, "amountTinybar");
}

function validateCreateScheduledTransferInput(
  input: CreateScheduledTransferInput,
): void {
  if (!input.fromAccountId.trim()) {
    throw invalidConfig("fromAccountId must not be empty");
  }

  if (!input.toAccountId.trim()) {
    throw invalidConfig("toAccountId must not be empty");
  }

  if (input.payerAccountId !== undefined && !input.payerAccountId.trim()) {
    throw invalidConfig("payerAccountId must not be empty when provided");
  }

  validateTinybarString(input.amountTinybar, "amountTinybar");
}

function validateSignScheduleInput(input: SignScheduleInput): void {
  if (!input.scheduleId.trim()) {
    throw invalidConfig("scheduleId must not be empty");
  }

  if (!input.signerPrivateKey.trim()) {
    throw invalidConfig("signerPrivateKey must not be empty");
  }
}

export async function createClient(
  options: CreateClientOptions,
): Promise<HieroRuntimeClient> {
  const config = normalizeRuntimeConfig(options);

  try {
    const native = await NativeRuntime.create(JSON.stringify(config));
    return new HieroRuntimeClient(native, config);
  } catch (error) {
    throw HieroRuntimeError.fromUnknown(error);
  }
}
