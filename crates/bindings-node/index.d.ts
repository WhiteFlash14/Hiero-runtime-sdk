export declare class NativeRuntime {
  /**
   * Create a runtime instance.
   */
  static create(configJson: string): Promise<NativeRuntime>
  getMirrorTransaction(transactionId: string): Promise<string>
  listTransactionsForAccount(accountId: string, limit?: number | undefined | null, cursor?: string | undefined | null): Promise<string>
  submitHbarTransfer(requestJson: string): Promise<string>
  waitForReceipt(transactionId: string): Promise<string>
  waitForFinality(transactionId: string): Promise<string>
  createScheduledTransfer(requestJson: string): Promise<string>
  signSchedule(requestJson: string): Promise<string>
  getSchedule(scheduleId: string): Promise<string>
  waitForScheduleExecution(scheduleId: string): Promise<string>
  getMirrorAccount(id: string): Promise<string>
  getContractResult(transactionIdOrHash: string, nonce?: number | undefined | null): Promise<string>
  deleteSchedule(scheduleId: string): Promise<void>
}

export declare function getAddonMetadata(): string
