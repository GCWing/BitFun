import type {
  ErrorData,
  InitializeResult,
  PermissionRespondResult,
  QueryCancelResult,
  QueryEventParams,
  QueryResultParams,
  QueryStartResult,
  SessionCloseResult,
  SessionCreateResult,
  ShutdownResult,
} from "./wire/index.js";
import {
  isErrorData as isGeneratedErrorData,
  isInitializeResult,
  isPermissionRespondResult,
  isQueryCancelResult,
  isQueryEventParams,
  isQueryResultParams,
  isQueryStartResult,
  isSessionCloseResult,
  isSessionCreateResult,
  isShutdownResult,
} from "./wire-validators.js";

// Rust protocol types own field names, shapes, and enum values through the
// generated validators. This module keeps only SDK envelope and semantic rules
// that TypeScript types cannot express.

export type QueryNotification =
  | { jsonrpc: "2.0"; method: "query/event"; params: QueryEventParams }
  | { jsonrpc: "2.0"; method: "query/result"; params: QueryResultParams };

export function validateResponseResult<T>(method: string, value: unknown): T {
  switch (method) {
    case "initialize":
      return validateInitializeResult(value) as T;
    case "session/create":
    case "session/resume":
      return validateSessionCreateResult(value) as T;
    case "query/start":
      return validateQueryStartResult(value) as T;
    case "query/cancel":
      return validateQueryCancelResult(value) as T;
    case "permission/respond":
      return validatePermissionRespondResult(value) as T;
    case "session/close":
      return validateSessionCloseResult(value) as T;
    case "shutdown":
      return validateWireValue(isShutdownResult, value, "shutdown result") as T;
    default:
      throw new Error(`SDK Host returned a result for unknown method ${method}`);
  }
}

export function validateQueryNotification(value: unknown): QueryNotification {
  assertRecord(value, "Query notification");
  assertOnlyKeys(value, ["jsonrpc", "method", "params"], "Query notification");
  if (value.jsonrpc !== "2.0") {
    throw new Error("Query notification has an invalid JSON-RPC version");
  }
  if (value.method === "query/event") {
    return {
      jsonrpc: "2.0",
      method: "query/event",
      params: validateQueryEventParams(value.params),
    };
  }
  if (value.method === "query/result") {
    return {
      jsonrpc: "2.0",
      method: "query/result",
      params: validateQueryResultParams(value.params),
    };
  }
  throw new Error("SDK Host notification method is unsupported");
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function hasOnlyKeys(
  value: Record<string, unknown>,
  allowed: readonly string[],
): boolean {
  const keys = Object.keys(value);
  return keys.length <= allowed.length && keys.every((key) => allowed.includes(key));
}

export function isErrorData(value: unknown): value is ErrorData {
  return isGeneratedErrorData(value);
}

function validateInitializeResult(value: unknown): InitializeResult {
  const result = validateWireValue(
    isInitializeResult,
    value,
    "initialize result",
  );
  if (
    !Number.isSafeInteger(result.protocolVersion) ||
    !isNonEmptyString(result.modelId)
  ) {
    throw new Error("SDK Host initialize protocol version or model id is invalid");
  }
  return result;
}

function validateSessionCreateResult(value: unknown): SessionCreateResult {
  const result = validateWireValue(
    isSessionCreateResult,
    value,
    "Session create result",
  );
  if (!isNonEmptyString(result.sessionId)) {
    throw new Error("SDK Host Session create identity is invalid");
  }
  return result;
}

function validateQueryStartResult(value: unknown): QueryStartResult {
  const result = validateWireValue(isQueryStartResult, value, "Query start result");
  if (!hasQueryIdentity(result)) {
    throw new Error("SDK Host Query start identity is invalid");
  }
  return result;
}

function validateQueryCancelResult(value: unknown): QueryCancelResult {
  const result = validateWireValue(
    isQueryCancelResult,
    value,
    "Query cancel result",
  );
  if (!hasQueryIdentity(result)) {
    throw new Error("SDK Host Query cancel identity is invalid");
  }
  return result;
}

function validatePermissionRespondResult(value: unknown): PermissionRespondResult {
  const result = validateWireValue(
    isPermissionRespondResult,
    value,
    "permission response result",
  );
  if (!isNonEmptyString(result.requestId) || !result.accepted) {
    throw new Error("SDK Host permission response result is invalid");
  }
  return result;
}

function validateSessionCloseResult(value: unknown): SessionCloseResult {
  const result = validateWireValue(
    isSessionCloseResult,
    value,
    "Session close result",
  );
  if (!isNonEmptyString(result.sessionId)) {
    throw new Error("SDK Host Session close identity is invalid");
  }
  return result;
}

function validateQueryEventParams(value: unknown): QueryEventParams {
  const params = validateWireValue(isQueryEventParams, value, "Query event params");
  if (
    !hasQueryIdentity(params) ||
    !Number.isSafeInteger(params.sequence) ||
    params.sequence < 1
  ) {
    throw new Error("SDK Host Query event identity or sequence is invalid");
  }
  return params;
}

function validateQueryResultParams(value: unknown): QueryResultParams {
  const params = validateWireValue(
    isQueryResultParams,
    value,
    "Query result params",
  );
  if (!hasQueryIdentity(params)) {
    throw new Error("SDK Host Query result identity is invalid");
  }
  if (params.status === "completed" && params.error !== undefined) {
    throw new Error("A completed SDK Host Query Result cannot contain an error");
  }
  return params;
}

function validateWireValue<T>(
  validator: (value: unknown) => value is T,
  value: unknown,
  label: string,
): T {
  if (!validator(value)) {
    throw new Error(`${label} does not match the generated Rust wire contract`);
  }
  return value;
}

function hasQueryIdentity(value: {
  queryId: string;
  sessionId: string;
  turnId: string;
  operationId: string;
}): boolean {
  return (
    isNonEmptyString(value.queryId) &&
    isNonEmptyString(value.sessionId) &&
    isNonEmptyString(value.turnId) &&
    isNonEmptyString(value.operationId)
  );
}

function assertRecord(
  value: unknown,
  label: string,
): asserts value is Record<string, unknown> {
  if (!isRecord(value)) {
    throw new Error(`${label} must be an object`);
  }
}

function assertOnlyKeys(
  value: Record<string, unknown>,
  allowed: readonly string[],
  label: string,
): void {
  if (!hasOnlyKeys(value, allowed)) {
    throw new Error(`${label} contains unknown fields`);
  }
}

function isNonEmptyString(value: string): boolean {
  return value.length > 0;
}
