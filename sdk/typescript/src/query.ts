import type { JsonRpcConnection } from "./internal/json-rpc.js";
import { withTimeout } from "./internal/deadline.js";
import type {
  PermissionRespondParams,
  PermissionRespondResult,
  QueryCancelParams,
  QueryCancelResult,
  QueryEventParams,
  QueryResultParams,
  QueryStartResult,
  SessionCloseParams,
  SessionCloseResult,
} from "./internal/wire/index.js";
import type {
  PermissionResponse,
  QueryStreamItem,
  Result,
  ResultError,
  Turn,
  Usage,
} from "./types.js";
import { isConnectionUnusableError, SdkError } from "./errors.js";

interface QueueWaiter {
  resolve(value: IteratorResult<QueryStreamItem>): void;
  reject(error: Error): void;
}

const MAX_BUFFERED_ITEMS = 1_024;
const MAX_BUFFERED_EVENT_BYTES = 1024 * 1024;

export class Query implements AsyncIterable<QueryStreamItem> {
  readonly #connection: JsonRpcConnection;
  readonly id: string;
  readonly operationId: string;
  readonly turn: Turn;
  readonly #items: QueryStreamItem[] = [];
  readonly #waiters: QueueWaiter[] = [];
  readonly #resultPromise: Promise<Result>;
  readonly #resolveResult: (result: Result) => void;
  readonly #rejectResult: (error: Error) => void;
  readonly #ownsSession: boolean;
  readonly #closeTimeoutMs: number;
  readonly #closedHandlers = new Set<() => void>();
  readonly #pendingPermissionIds = new Set<string>();
  #lastSequence = 0;
  #terminal = false;
  #unsubscribe: () => void = () => {};
  #cancelPromise?: Promise<void>;
  #closePromise?: Promise<void>;
  #failure?: Error;
  #closed = false;
  #bufferedBytes = 0;

  /** @internal */
  static create(
    connection: JsonRpcConnection,
    started: QueryStartResult,
    closeTimeoutMs = 7_000,
  ): Query {
    return new Query(connection, started, closeTimeoutMs);
  }

  private constructor(
    connection: JsonRpcConnection,
    started: QueryStartResult,
    closeTimeoutMs: number,
  ) {
    this.#connection = connection;
    this.id = started.queryId;
    this.operationId = started.operationId;
    this.#ownsSession = started.createdSession;
    this.#closeTimeoutMs = closeTimeoutMs;
    this.turn = Object.freeze({
      id: started.turnId,
      sessionId: started.sessionId,
    });
    let resolveResult!: (result: Result) => void;
    let rejectResult!: (error: Error) => void;
    this.#resultPromise = new Promise((resolve, reject) => {
      resolveResult = resolve;
      rejectResult = reject;
    });
    this.#resolveResult = resolveResult;
    this.#rejectResult = rejectResult;
    this.#unsubscribe = connection.subscribeQuery(
      this.id,
      (notification) => {
        if (notification.method === "query/event") {
          this.#onEvent(notification.params);
        } else {
          this.#onResult(notification.params);
        }
      },
      (error) => this.#onConnectionFailure(error),
    );
    if (this.#terminal) {
      this.#unsubscribe();
    }
  }

  result(): Promise<Result> {
    return this.#resultPromise;
  }

  cancel(): Promise<void> {
    if (this.#terminal) {
      return Promise.resolve();
    }
    this.#cancelPromise ??= this.#requestCancel();
    return this.#cancelPromise;
  }

  async respondPermission(
    requestId: string,
    response: PermissionResponse,
  ): Promise<void> {
    if (!this.#pendingPermissionIds.delete(requestId)) {
      throw new Error("Permission request is unknown, expired, or already answered");
    }
    if (response.decision !== "reject" && response.feedback !== undefined) {
      throw new Error("Permission feedback is only valid when rejecting a request");
    }
    const params: PermissionRespondParams = {
      queryId: this.id,
      sessionId: this.turn.sessionId,
      turnId: this.turn.id,
      operationId: this.operationId,
      requestId,
      decision: response.decision,
      feedback: response.feedback,
    };
    const result = await this.#connection.request<PermissionRespondResult>(
      "permission/respond",
      params,
    );
    if (result.requestId !== requestId || !result.accepted) {
      throw new Error("SDK Host answered a different permission request");
    }
  }

  close(): Promise<void> {
    this.#closePromise ??= this.#closeQuery();
    return this.#closePromise;
  }

  /** @internal */
  get ownsSession(): boolean {
    return this.#ownsSession;
  }

  /** @internal */
  onClosed(handler: () => void): void {
    this.#closedHandlers.add(handler);
  }

  [Symbol.asyncDispose](): Promise<void> {
    return this.close();
  }

  [Symbol.asyncIterator](): AsyncIterator<QueryStreamItem> {
    return {
      next: () => this.#next(),
      return: async () => {
        await this.close();
        return { done: true, value: undefined };
      },
    };
  }

  #onEvent(params: QueryEventParams): void {
    this.#assertIdentity(
      params.queryId,
      params.sessionId,
      params.turnId,
      params.operationId,
    );
    if (this.#terminal || params.sequence !== this.#lastSequence + 1) {
      throw new Error("SDK Host Query event ordering is invalid");
    }
    this.#lastSequence = params.sequence;
    if (params.event.type === "assistant_text_delta") {
      this.#push({
        type: "assistant_text_delta",
        queryId: params.queryId,
        sessionId: params.sessionId,
        turnId: params.turnId,
        operationId: params.operationId,
        sequence: params.sequence,
        text: params.event.text,
      });
    } else if (params.event.type === "tool_event") {
      this.#push({
        type: "tool_event",
        queryId: params.queryId,
        sessionId: params.sessionId,
        turnId: params.turnId,
        operationId: params.operationId,
        sequence: params.sequence,
        toolCallId: params.event.toolCallId,
        toolName: params.event.toolName,
        status: params.event.status,
        ...(params.event.progress === undefined
          ? {}
          : { progress: params.event.progress }),
        ...(params.event.durationMs === undefined
          ? {}
          : { durationMs: params.event.durationMs }),
      });
    } else {
      if (this.#pendingPermissionIds.has(params.event.requestId)) {
        throw new Error("SDK Host repeated a pending permission request");
      }
      this.#pendingPermissionIds.add(params.event.requestId);
      this.#push({
        type: "permission_request",
        queryId: params.queryId,
        sessionId: params.sessionId,
        turnId: params.turnId,
        operationId: params.operationId,
        sequence: params.sequence,
        requestId: params.event.requestId,
        action: params.event.action,
        resources: params.event.resources,
        source: params.event.source,
        toolCallId: params.event.toolCallId ?? undefined,
        responseTimeoutMs: params.event.responseTimeoutMs,
      });
    }
  }

  async #requestCancel(): Promise<void> {
    const params: QueryCancelParams = {
      queryId: this.id,
      sessionId: this.turn.sessionId,
      turnId: this.turn.id,
      operationId: this.operationId,
    };
    const cancelled = await this.#connection.request<QueryCancelResult>(
      "query/cancel",
      params,
    );
    if (
      cancelled.queryId !== this.id ||
      cancelled.sessionId !== this.turn.sessionId ||
      cancelled.turnId !== this.turn.id ||
      cancelled.operationId !== this.operationId
    ) {
      throw new Error("SDK Host returned a different Query from cancellation");
    }
  }

  async #closeQuery(): Promise<void> {
    const timeoutError = new SdkError(
      "SDK Host did not settle Query cleanup before its deadline",
      {
        code: "cleanup_required",
        stage: "query",
        retryable: false,
        correlationId: `query:${this.id}`,
        operationId: this.operationId,
        outcomeCertainty: "unknown",
        recovery: "restart_host",
      },
    );
    try {
      await withTimeout(
        this.#closeWithinProtocol(),
        this.#closeTimeoutMs,
        () => timeoutError,
      );
    } catch (error) {
      if (isConnectionUnusableError(error)) {
        await this.#connection.abort(error);
      }
      throw error;
    }
    this.#markClosed();
  }

  async #closeWithinProtocol(): Promise<void> {
    if (!this.#terminal) {
      await this.cancel();
    }
    await this.#resultPromise;
    if (this.#ownsSession) {
      const params: SessionCloseParams = { sessionId: this.turn.sessionId };
      const closed = await this.#connection.request<SessionCloseResult>(
        "session/close",
        params,
      );
      if (closed.sessionId !== this.turn.sessionId) {
        throw new Error("SDK Host closed a different transient Session");
      }
    }
  }

  #onResult(params: QueryResultParams): void {
    this.#assertIdentity(
      params.queryId,
      params.sessionId,
      params.turnId,
      params.operationId,
    );
    if (this.#terminal) {
      throw new Error("SDK Host emitted more than one terminal Result");
    }
    const result: Result = {
      type: "result",
      queryId: params.queryId,
      sessionId: params.sessionId,
      turnId: params.turnId,
      operationId: params.operationId,
      status: params.status,
      outputText: params.output.text,
      ...(params.output.structured === undefined
        ? {}
        : { structuredOutput: params.output.structured }),
      ...(params.usage === undefined ? {} : { usage: mapUsage(params.usage) }),
      ...(params.error === undefined ? {} : { error: mapResultError(params.error) }),
    };
    if (!this.#push(result)) {
      return;
    }
    this.#terminal = true;
    this.#pendingPermissionIds.clear();
    this.#resolveResult(result);
    this.#unsubscribe();
    while (this.#waiters.length > 0) {
      this.#waiters.shift()?.resolve({ done: true, value: undefined });
    }
  }

  #assertIdentity(
    queryId: string,
    sessionId: string,
    turnId: string,
    operationId: string,
  ): void {
    if (
      queryId !== this.id ||
      sessionId !== this.turn.sessionId ||
      turnId !== this.turn.id ||
      operationId !== this.operationId
    ) {
      throw new Error("SDK Host Query identity changed during streaming");
    }
  }

  #push(item: QueryStreamItem): boolean {
    const waiter = this.#waiters.shift();
    if (waiter !== undefined) {
      waiter.resolve({ done: false, value: item });
      return true;
    }
    const itemBytes = bufferedItemBytes(item);
    if (
      this.#items.length >= MAX_BUFFERED_ITEMS ||
      this.#bufferedBytes + itemBytes > MAX_BUFFERED_EVENT_BYTES
    ) {
      const error = new SdkError("SDK Host Query event buffer capacity was exceeded", {
        code: "overloaded",
        stage: "query",
        retryable: false,
        correlationId: `query:${this.id}`,
        operationId: this.operationId,
        outcomeCertainty: "unknown",
        recovery: "restart_host",
      });
      void this.#connection.abort(error);
      return false;
    }
    this.#items.push(item);
    this.#bufferedBytes += itemBytes;
    return true;
  }

  #next(): Promise<IteratorResult<QueryStreamItem>> {
    const item = this.#items.shift();
    if (item !== undefined) {
      this.#bufferedBytes -= bufferedItemBytes(item);
      return Promise.resolve({ done: false, value: item });
    }
    if (this.#failure !== undefined) {
      return Promise.reject(this.#failure);
    }
    if (this.#terminal) {
      return Promise.resolve({ done: true, value: undefined });
    }
    return new Promise((resolve, reject) => {
      this.#waiters.push({ resolve, reject });
    });
  }

  #onConnectionFailure(error: SdkError): void {
    if (this.#terminal) {
      return;
    }
    this.#terminal = true;
    this.#pendingPermissionIds.clear();
    this.#failure = error;
    this.#rejectResult(error);
    this.#unsubscribe();
    while (this.#waiters.length > 0) {
      this.#waiters.shift()?.reject(error);
    }
  }

  #markClosed(): void {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    for (const handler of this.#closedHandlers) {
      handler();
    }
    this.#closedHandlers.clear();
  }
}

function mapUsage(usage: NonNullable<QueryResultParams["usage"]>): Usage {
  return {
    inputTokens: usage.inputTokens,
    ...(usage.outputTokens === undefined
      ? {}
      : { outputTokens: usage.outputTokens }),
    totalTokens: usage.totalTokens,
    ...(usage.cachedTokens === undefined
      ? {}
      : { cachedTokens: usage.cachedTokens }),
  };
}

function bufferedItemBytes(item: QueryStreamItem): number {
  // Result frames have their own connection-level frame bound. Counting their
  // aggregate output again against the event backlog would reject a valid
  // Query that has not consumed the same text deltas yet.
  return item.type === "result"
    ? 0
    : Buffer.byteLength(JSON.stringify(item), "utf8") + 128;
}

function mapResultError(error: QueryResultParams["error"]): ResultError {
  if (error === undefined) {
    throw new Error("Result error is unavailable");
  }
  return {
    message: error.message,
    code: error.data.code,
    stage: error.data.stage,
    retryable: error.data.retryable,
    correlationId: error.data.correlationId,
    operationId: error.data.operationId ?? undefined,
    causationId: error.data.causationId ?? undefined,
    outcomeCertainty: error.data.outcomeCertainty,
    recovery: error.data.recovery ?? undefined,
  };
}
