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
  /** Advanced absolute-path override for the package-local native Host. */
  hostPath?: string;
  /** Deadline for the SDK Host initialize handshake. */
  initializeTimeoutMs?: number;
  /** Process-lifetime model credentials installed into this Host connection. */
  model: AgentModelOptions;
}

export interface AgentCapabilities {
  query: boolean;
  sessions: boolean;
  cancellation: boolean;
  eventStream: boolean;
  toolEvents: boolean;
  imageInput: boolean;
  permissionResponses: boolean;
  structuredOutput: boolean;
  usage: boolean;
  customTools: boolean;
  hooks: boolean;
  mcpConfiguration: boolean;
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

export type SessionLifetime = "connection" | "durable";

export type UserInput =
  | { type: "text"; text: string }
  | { type: "local_image"; path: string };

export type Input = string | readonly UserInput[];

export type JsonSchema = Record<string, unknown>;

export interface QueryInput {
  prompt: Input;
  agent?: string;
  outputSchema?: JsonSchema;
}

export interface SessionCreateInput {
  sessionName?: string;
  agent?: string;
}

export interface TurnInput {
  prompt: Input;
  outputSchema?: JsonSchema;
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

export interface ToolEvent {
  type: "tool_event";
  queryId: string;
  sessionId: string;
  turnId: string;
  operationId: string;
  sequence: number;
  toolCallId: string;
  toolName: string;
  status: "started" | "progress" | "completed" | "failed" | "cancelled";
  progress?: number;
  durationMs?: number;
}

export interface PermissionSource {
  kind: "tool_call" | "provider" | "extension";
  identity: string;
}

export interface PermissionRequestEvent {
  type: "permission_request";
  queryId: string;
  sessionId: string;
  turnId: string;
  operationId: string;
  sequence: number;
  requestId: string;
  action: string;
  resources: readonly string[];
  source: PermissionSource;
  toolCallId?: string;
  responseTimeoutMs: number;
}

export type PermissionDecision = "allow_once" | "allow_always" | "reject";

export interface PermissionResponse {
  decision: PermissionDecision;
  feedback?: string;
}

export type ResultStatus = "completed" | "failed" | "cancelled";

export interface ResultError extends SdkErrorDetails {
  message: string;
}

export interface Usage {
  inputTokens: number;
  outputTokens?: number;
  totalTokens: number;
  cachedTokens?: number;
}

export interface Result {
  type: "result";
  queryId: string;
  sessionId: string;
  turnId: string;
  operationId: string;
  status: ResultStatus;
  outputText: string;
  structuredOutput?: unknown;
  usage?: Usage;
  error?: ResultError;
}

export type QueryStreamItem =
  | AssistantTextDelta
  | ToolEvent
  | PermissionRequestEvent
  | Result;
