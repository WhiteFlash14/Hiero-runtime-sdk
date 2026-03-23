export type NetworkKind = "mainnet" | "testnet" | "previewnet" | "custom";

export type RuntimeErrorCode =
  | "INVALID_CONFIG"
  | "TRANSPORT"
  | "MIRROR_HTTP"
  | "CONSENSUS"
  | "SCHEDULE"
  | "TIMEOUT"
  | "RATE_LIMITED"
  | "NOT_FOUND"
  | "SERIALIZATION"
  | "UNSUPPORTED"
  | "INTERNAL";

/// A single consensus node used for custom-network configurations.
export interface ConsensusNodeConfig {
  url: string;
  accountId: string;
}

export interface NetworkConfig {
  kind: NetworkKind;
  mirrorBaseUrl: string;
  consensusNodes?: ConsensusNodeConfig[];
}

export interface OperatorConfig {
  accountId: string;
  privateKey: string;
}

export interface RetryPolicy {
  maxAttempts: number;
  initialDelayMs: number;
  maxDelayMs: number;
  jitter: boolean;
}

export interface FinalityPolicy {
  receiptTimeoutMs: number;
  mirrorTimeoutMs: number;
  pollIntervalMs: number;
}

export interface RuntimeConfig {
  network: NetworkConfig;
  operator?: OperatorConfig;
  retry: RetryPolicy;
  finality: FinalityPolicy;
}

export interface CreateClientOptions {
  network: NetworkKind;
  mirror?: {
    baseUrl?: string;
  };
  consensusNodes?: ConsensusNodeConfig[];
  operator?: OperatorConfig;
  retry?: Partial<RetryPolicy>;
  finality?: Partial<FinalityPolicy>;
}

export interface RuntimeErrorPayload {
  code: RuntimeErrorCode;
  message: string;
  retryable: boolean;
  details?: unknown;
}

export interface MirrorTransactionRecord {
  transactionId: string;
  result: string;
  consensusTimestamp?: string;
  name?: string;
  scheduled?: boolean;
  nonce?: number;
}

export interface MirrorAccountView {
  account: string;
  balance: string;
  evmAddress?: string;
  deleted: boolean;
  memo: string;
}

export interface ContractResultView {
  contractId?: string;
  transactionId: string;
  result: string;
  status: string;
  gasUsed: string;
  errorMessage?: string;
  callResult?: string;
  from?: string;
  to?: string;
}

export interface TransactionLookup {
  requestedTransactionId: string;
  primary: MirrorTransactionRecord;
  duplicates: MirrorTransactionRecord[];
  entries: MirrorTransactionRecord[];
}

export interface SubmittedTransaction {
  transactionId: string;
}

export interface HbarTransferInput {
  fromAccountId: string;
  toAccountId: string;
  amountTinybar: string;
}

export interface ReceiptResult {
  transactionId: string;
  status: string;
}

export interface FinalizedTransaction {
  transactionId: string;
  receipt: ReceiptResult;
  primaryMirrorEntry?: MirrorTransactionRecord;
  duplicates: MirrorTransactionRecord[];
}

export type ScheduleState =
  | "pendingSignatures"
  | "executed"
  | "expired"
  | "deleted";

export interface CreatedSchedule {
  scheduleId: string;
  scheduledTransactionId: string;
  status: ScheduleState;
}

export interface ScheduleInfoView {
  scheduleId: string;
  payerAccountId?: string;
  creatorAccountId?: string;
  signatories: string[];
  scheduledTransactionId?: string;
  status: ScheduleState;
  expirationTime?: string;
  executedTimestamp?: string;
  deletionTimestamp?: string;
}

export interface ScheduleExecution {
  scheduleId: string;
  scheduledTransactionId: string;
  finalized: FinalizedTransaction;
}

export interface CreateScheduledTransferInput {
  fromAccountId: string;
  toAccountId: string;
  payerAccountId?: string;
  amountTinybar: string;
  memo?: string;
}

export interface DeleteScheduleInput {
  scheduleId: string;
}

export interface SignScheduleInput {
  scheduleId: string;
  signerPrivateKey: string;
}

/** A single page of results from a paginated Mirror Node list query. */
export interface PageResult<T> {
  items: T[];
  nextCursor?: string;
}


export interface ListTransactionsOptions {
  limit?: number;
  cursor?: string;
}

export interface AttachedTransactionHandle {
  transactionId: string;
  waitForReceipt(): Promise<ReceiptResult>;
  waitForFinality(): Promise<FinalizedTransaction>;
}
