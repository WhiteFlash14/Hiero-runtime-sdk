import type { RuntimeErrorCode, RuntimeErrorPayload } from "./types.js";

const DEFAULT_ERROR_CODE: RuntimeErrorCode = "INTERNAL";

function isRuntimeErrorPayload(value: unknown): value is RuntimeErrorPayload {
  if (!value || typeof value !== "object") {
    return false;
  }

  const record = value as Record<string, unknown>;

  return (
    typeof record.code === "string" &&
    typeof record.message === "string" &&
    typeof record.retryable === "boolean"
  );
}

export class HieroRuntimeError extends Error {
  readonly code: RuntimeErrorCode;
  readonly retryable: boolean;
  readonly details?: unknown;

  constructor(payload: RuntimeErrorPayload) {
    super(payload.message);
    this.name = "HieroRuntimeError";
    this.code = payload.code;
    this.retryable = payload.retryable;
    this.details = payload.details;
  }

  static fromUnknown(error: unknown): HieroRuntimeError {
    if (error instanceof HieroRuntimeError) {
      return error;
    }

    if (error instanceof Error) {
      const parsed = tryParseRuntimeErrorPayload(error.message);
      if (parsed) {
        return new HieroRuntimeError(parsed);
      }

      return new HieroRuntimeError({
        code: DEFAULT_ERROR_CODE,
        message: error.message,
        retryable: false,
      });
    }

    return new HieroRuntimeError({
      code: DEFAULT_ERROR_CODE,
      message: String(error),
      retryable: false,
    });
  }
}

export function tryParseRuntimeErrorPayload(
  raw: string,
): RuntimeErrorPayload | null {
  try {
    const parsed: unknown = JSON.parse(raw);
    return isRuntimeErrorPayload(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export function invalidConfig(
  message: string,
  details?: unknown,
): HieroRuntimeError {
  return new HieroRuntimeError({
    code: "INVALID_CONFIG",
    message,
    retryable: false,
    details,
  });
}
