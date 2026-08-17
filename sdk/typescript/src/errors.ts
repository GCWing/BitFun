import type {
  OutcomeCertainty,
  RecoveryAction,
  SdkErrorCode,
  SdkErrorDetails,
  SdkErrorStage,
} from "./types.js";

export class SdkError extends Error {
  readonly code: SdkErrorCode;
  readonly stage: SdkErrorStage;
  readonly retryable: boolean;
  readonly correlationId: string;
  readonly operationId?: string;
  readonly causationId?: string;
  readonly outcomeCertainty: OutcomeCertainty;
  readonly recovery?: RecoveryAction;

  constructor(message: string, data: SdkErrorDetails, options?: ErrorOptions) {
    super(message, options);
    this.name = "SdkError";
    this.code = data.code;
    this.stage = data.stage;
    this.retryable = data.retryable;
    this.correlationId = data.correlationId;
    this.operationId = data.operationId ?? undefined;
    this.causationId = data.causationId ?? undefined;
    this.outcomeCertainty = data.outcomeCertainty;
    this.recovery = data.recovery ?? undefined;
  }
}

/** @internal */
export function isConnectionUnusableError(error: unknown): error is SdkError {
  return (
    error instanceof SdkError &&
    error.outcomeCertainty === "unknown" &&
    error.recovery === "restart_host"
  );
}
