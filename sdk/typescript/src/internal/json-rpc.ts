import {
  JsonRpcLimitError,
  JsonRpcPeer,
  JsonRpcProtocolError,
  JsonRpcRemoteError,
  JsonRpcTimeoutError,
  JsonRpcTransportError,
  type JsonRpcLimits,
  type JsonRpcNotification,
} from "../../../../src/crates/adapters/transport/typescript/src/index.js";

import { SdkError } from "../errors.js";
import type { SdkErrorStage } from "../types.js";
import { withTimeout } from "./deadline.js";
import type { HostTransport } from "./transport.js";
import { HostMessageTransport } from "./transport.js";
import {
  isErrorData,
  type QueryNotification,
  validateQueryNotification,
  validateResponseResult,
} from "./wire-validation.js";

interface QuerySubscriber {
  notification(value: QueryNotification): void;
  failure(error: SdkError): void;
}

interface JsonRpcConnectionOptions {
  requestTimeoutMs?: number;
  limits?: Partial<JsonRpcLimits>;
}

const DEFAULT_LIMITS: JsonRpcLimits = {
  maxMessageBytes: 1024 * 1024,
  maxPendingRequests: 128,
  maxPendingBytes: 8 * 1024 * 1024,
  maxOutboundBytes: 2 * 1024 * 1024,
};

const DEFAULT_REQUEST_TIMEOUT_MS = 30_000;

export class JsonRpcConnection {
  readonly #peer: JsonRpcPeer;
  readonly #queryHandlers = new Map<string, QuerySubscriber>();
  readonly #backlog = new Map<string, QueryNotification[]>();
  #closed = false;
  #transportClosePromise?: Promise<void>;

  constructor(
    transport: HostTransport,
    options: JsonRpcConnectionOptions = {},
  ) {
    const limits: JsonRpcLimits = {
      ...DEFAULT_LIMITS,
      ...options.limits,
    };
    let nextRequestId = 1;
    this.#peer = new JsonRpcPeer(
      new HostMessageTransport(transport, limits.maxMessageBytes),
      {
        createRequestId: () => nextRequestId++,
        requestTimeoutMs: options.requestTimeoutMs ?? DEFAULT_REQUEST_TIMEOUT_MS,
        limits,
        onNotificationError: (error) => {
          this.#fail(
            processLost("SDK Host notification violates the protocol", error),
          );
        },
      },
    );
    this.#peer.onNotification((notification) => {
      this.#handleNotification(notification);
    });
    this.#peer.onFailure((error) => {
      this.#handlePeerFailure(error);
    });
  }

  async request<T>(
    method: string,
    params: object,
    timeoutMs?: number,
  ): Promise<T> {
    if (this.#closed) {
      throw processLost("SDK Host connection is closed");
    }
    let result: unknown;
    try {
      result = await this.#peer.request(method, params, timeoutMs);
    } catch (error) {
      if (error instanceof JsonRpcRemoteError) {
        if (!isErrorData(error.data)) {
          const protocolError = processLost(
            "SDK Host returned an invalid error contract",
            error,
          );
          await this.abort(protocolError);
          throw protocolError;
        }
        throw new SdkError(error.message, error.data, { cause: error });
      }
      if (error instanceof JsonRpcLimitError) {
        throw requestOverloaded(method, error);
      }
      if (error instanceof JsonRpcTimeoutError) {
        const timeoutError = requestTimeout(method, error);
        await this.abort(timeoutError);
        throw timeoutError;
      }
      if (
        error instanceof JsonRpcProtocolError ||
        error instanceof JsonRpcTransportError
      ) {
        throw processLost("SDK Host connection failed", error);
      }
      throw error;
    }

    try {
      return validateResponseResult(method, result) as T;
    } catch (error) {
      const protocolError = processLost(
        "SDK Host returned an invalid method result",
        error,
      );
      await this.abort(protocolError);
      throw protocolError;
    }
  }

  subscribeQuery(
    queryId: string,
    handler: (value: QueryNotification) => void,
    onFailure: (error: SdkError) => void,
  ): () => void {
    if (this.#queryHandlers.has(queryId)) {
      throw new Error(`Query ${queryId} already has a stream owner`);
    }
    this.#queryHandlers.set(queryId, {
      notification: handler,
      failure: onFailure,
    });
    for (const notification of this.#backlog.get(queryId) ?? []) {
      handler(notification);
    }
    this.#backlog.delete(queryId);
    return () => {
      this.#queryHandlers.delete(queryId);
      this.#backlog.delete(queryId);
    };
  }

  async shutdown(timeoutMs = 12_000): Promise<void> {
    if (this.#closed) {
      await this.#closeTransport();
      return;
    }
    try {
      const timeoutError = cleanupTimeout(
        "SDK Host did not acknowledge shutdown before its cleanup deadline",
        "shutdown",
        "local:shutdown_timeout",
      );
      try {
        await withTimeout(this.request("shutdown", {}), timeoutMs, () => timeoutError);
      } catch (error) {
        if (error === timeoutError) {
          this.#fail(timeoutError);
        }
        throw error;
      }
    } finally {
      this.#closed = true;
      await this.#closeTransport();
    }
  }

  /** @internal */
  abort(error: SdkError): Promise<void> {
    this.#fail(error);
    return this.#closeTransport();
  }

  #handleNotification(notification: JsonRpcNotification): void {
    if (
      notification.method !== "query/event" &&
      notification.method !== "query/result"
    ) {
      throw new Error("Unsupported SDK Host notification");
    }
    const validated = validateQueryNotification({
      jsonrpc: "2.0",
      method: notification.method,
      params: notification.params,
    });
    const handler = this.#queryHandlers.get(validated.params.queryId);
    if (handler !== undefined) {
      handler.notification(validated);
      return;
    }
    const backlog = this.#backlog.get(validated.params.queryId) ?? [];
    if (backlog.length >= 16) {
      throw new Error("SDK Host sent too many events before Query registration");
    }
    backlog.push(validated);
    this.#backlog.set(validated.params.queryId, backlog);
  }

  #handlePeerFailure(error: Error): void {
    if (this.#closed) {
      return;
    }
    this.#fail(
      error instanceof SdkError
        ? error
        : processLost("SDK Host connection was lost", error),
    );
  }

  #fail(error: SdkError): void {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    for (const subscriber of this.#queryHandlers.values()) {
      subscriber.failure(error);
    }
    this.#queryHandlers.clear();
    this.#backlog.clear();
    void this.#peer.abort(error);
  }

  #closeTransport(): Promise<void> {
    this.#transportClosePromise ??= this.#peer.close();
    return this.#transportClosePromise;
  }
}

function requestStage(method: string): SdkErrorStage {
  if (method === "initialize") {
    return "initialize";
  }
  if (method.startsWith("query/")) {
    return "query";
  }
  if (method.startsWith("session/")) {
    return "session";
  }
  if (method === "shutdown") {
    return "shutdown";
  }
  return "protocol";
}

function requestOverloaded(method: string, cause: JsonRpcLimitError): SdkError {
  return new SdkError(
    "SDK Host request capacity was exceeded before the request was sent",
    {
      code: "overloaded",
      stage: requestStage(method),
      retryable: true,
      correlationId: `local:${cause.limit}`,
      outcomeCertainty: "not_started",
    },
    { cause },
  );
}

function requestTimeout(method: string, cause: JsonRpcTimeoutError): SdkError {
  return new SdkError(`SDK Host request timed out: ${method}`, {
    code: "timeout",
    stage: requestStage(method),
    retryable: false,
    correlationId: `local:request_timeout:${String(cause.id)}`,
    outcomeCertainty: "unknown",
    recovery: "restart_host",
  }, { cause });
}

function processLost(message: string, cause?: unknown): SdkError {
  return new SdkError(message, {
    code: "process_lost",
    stage: "protocol",
    retryable: false,
    correlationId: "local:process_lost",
    outcomeCertainty: "unknown",
    recovery: "restart_host",
  }, cause === undefined ? undefined : { cause });
}

function cleanupTimeout(
  message: string,
  stage: "query" | "session" | "shutdown",
  correlationId: string,
  operationId?: string,
): SdkError {
  return new SdkError(message, {
    code: "cleanup_required",
    stage,
    retryable: false,
    correlationId,
    operationId,
    outcomeCertainty: "unknown",
    recovery: "restart_host",
  });
}
