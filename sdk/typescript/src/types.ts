export type AgentModelProvider =
  | "openai"
  | "responses"
  | "anthropic"
  | "gemini";

export interface AgentModelOptions {
  provider: AgentModelProvider;
  model: string;
  apiKey: string;
  baseUrl?: string;
}

export interface AgentClientOptions {
  cwd: string;
  /** Explicit absolute path to the native `bitfun-sdk-host`. */
  hostPath: string;
  /** Deadline for the SDK Host initialize handshake. */
  initializeTimeoutMs?: number;
  /** Process-lifetime model credentials installed into this Host connection. */
  model: AgentModelOptions;
}

export interface AgentCapabilities {
  query: boolean;
  sessions: boolean;
  cancellation: boolean;
}

export type SdkErrorCode =
  | "invalid_request"
  | "not_initialized"
  | "already_initialized"
  | "version_mismatch"
  | "capability_unavailable"
  | "not_found"
  | "permission_denied"
  | "action_required"
  | "authentication"
  | "rate_limited"
  | "provider_quota"
  | "provider_billing"
  | "provider_unavailable"
  | "context_overflow"
  | "content_policy"
  | "overloaded"
  | "timeout"
  | "cancelled"
  | "process_lost"
  | "cleanup_required"
  | "internal";

export type SdkErrorStage =
  | "protocol"
  | "initialize"
  | "session"
  | "query"
  | "shutdown";

export type OutcomeCertainty = "not_started" | "committed" | "unknown";

export type RecoveryAction =
  | "initialize"
  | "retry"
  | "update_sdk"
  | "restart_host";

export interface SdkErrorDetails {
  code: SdkErrorCode;
  stage: SdkErrorStage;
  retryable: boolean;
  correlationId: string;
  operationId?: string;
  causationId?: string;
  outcomeCertainty: OutcomeCertainty;
  recovery?: RecoveryAction;
}

export type SessionLifetime = "connection";

export interface QueryInput {
  prompt: string;
  agent?: string;
}

export interface SessionCreateInput {
  sessionName?: string;
  agent?: string;
}

export interface TurnInput {
  prompt: string;
}

export interface Turn {
  readonly id: string;
  readonly sessionId: string;
}

export interface AssistantTextDelta {
  type: "assistant_text_delta";
  queryId: string;
  sessionId: string;
  turnId: string;
  operationId: string;
  sequence: number;
  text: string;
}

export type ResultStatus = "completed" | "failed" | "cancelled";

export interface ResultError extends SdkErrorDetails {
  message: string;
}

export interface Result {
  type: "result";
  queryId: string;
  sessionId: string;
  turnId: string;
  operationId: string;
  status: ResultStatus;
  outputText: string;
  error?: ResultError;
}

export type QueryStreamItem = AssistantTextDelta | Result;
