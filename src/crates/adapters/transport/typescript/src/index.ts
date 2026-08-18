export type JsonRpcId = number | string;

export interface MessageTransportObserver {
  message(message: string): void;
  close(cause?: unknown): void;
}

/**
 * A carrier-neutral, one-message-at-a-time channel.
 *
 * Implementations own carrier framing and reconnect policy. A resolved `send`
 * means the carrier accepted the complete message; keeping the promise pending
 * is how a carrier projects backpressure into the shared JSON-RPC peer.
 */
export interface MessageTransport {
  subscribe(observer: MessageTransportObserver): () => void;
  send(message: string): Promise<void>;
  close(): Promise<void>;
}

export interface WebSocketLike {
  readonly readyState: number;
  readonly bufferedAmount: number;
  send(message: string): void;
  close(): void;
}

export interface WebSocketMessageTransportOptions {
  maxBufferedBytes: number;
  drainTimeoutMs: number;
}

export interface JsonRpcNotification {
  method: string;
  params: unknown;
}

export interface JsonRpcLimits {
  maxMessageBytes: number;
  maxPendingRequests: number;
  maxPendingBytes: number;
  maxOutboundBytes: number;
}

export interface JsonRpcPeerOptions {
  createRequestId(): JsonRpcId;
  requestTimeoutMs: number;
  limits: JsonRpcLimits;
  onNotificationError?(
    error: Error,
    notification: JsonRpcNotification,
  ): void;
}

export class JsonRpcProtocolError extends Error {
  override readonly name = "JsonRpcProtocolError";

  constructor(message: string, cause?: unknown) {
    super(message);
    (this as Error & { cause?: unknown }).cause = cause;
  }
}

export class JsonRpcTransportError extends Error {
  override readonly name = "JsonRpcTransportError";

  constructor(message: string, cause?: unknown) {
    super(message);
    (this as Error & { cause?: unknown }).cause = cause;
  }
}

export class JsonRpcRemoteError extends Error {
  override readonly name = "JsonRpcRemoteError";
  readonly code: number;
  readonly data: unknown;

  constructor(code: number, message: string, data: unknown) {
    super(message);
    this.code = code;
    this.data = data;
  }
}

export class JsonRpcTimeoutError extends Error {
  override readonly name = "JsonRpcTimeoutError";
  readonly method: string;
  readonly id: JsonRpcId;

  constructor(method: string, id: JsonRpcId) {
    super(`JSON-RPC request timed out: ${method}`);
    this.method = method;
    this.id = id;
  }
}

export type JsonRpcLimit =
  | "message_bytes"
  | "pending_count"
  | "pending_bytes"
  | "outbound_bytes";

export class JsonRpcLimitError extends Error {
  override readonly name = "JsonRpcLimitError";
  readonly limit: JsonRpcLimit;

  constructor(limit: JsonRpcLimit) {
    super(`JSON-RPC ${limit.replace(/_/g, " ")} limit was exceeded`);
    this.limit = limit;
  }
}

interface PendingRequest {
  retainedBytes: number;
  timer: ReturnType<typeof setTimeout>;
  resolve(value: unknown): void;
  reject(error: Error): void;
}

const UTF8 = new TextEncoder();
const WEB_SOCKET_OPEN = 1;
const WEB_SOCKET_CLOSING = 2;

/** Browser-compatible WebSocket message delivery with bounded backpressure. */
export class WebSocketMessageTransport implements MessageTransport {
  readonly #socket: WebSocketLike;
  readonly #options: WebSocketMessageTransportOptions;
  #observer?: MessageTransportObserver;
  #closePromise?: Promise<void>;
  #closed = false;

  constructor(
    socket: WebSocketLike,
    options: WebSocketMessageTransportOptions,
  ) {
    assertPositiveInteger(options.maxBufferedBytes, "WebSocket buffer limit");
    assertPositiveInteger(options.drainTimeoutMs, "WebSocket drain timeout");
    this.#socket = socket;
    this.#options = options;
  }

  subscribe(observer: MessageTransportObserver): () => void {
    if (this.#observer !== undefined) {
      throw new Error("WebSocket carrier already has a message owner");
    }
    this.#observer = observer;
    return () => {
      if (this.#observer === observer) {
        this.#observer = undefined;
      }
    };
  }

  async send(message: string): Promise<void> {
    if (this.#closed || this.#socket.readyState !== WEB_SOCKET_OPEN) {
      throw new Error("WebSocket is not connected");
    }
    const messageBytes = UTF8.encode(message).byteLength;
    if (
      this.#socket.bufferedAmount + messageBytes >
      this.#options.maxBufferedBytes
    ) {
      throw new Error("WebSocket send buffer capacity was exceeded");
    }
    this.#socket.send(message);
    await waitForWebSocketDrain(
      this.#socket,
      Date.now() + this.#options.drainTimeoutMs,
    );
  }

  receive(message: string): void {
    if (!this.#closed) {
      this.#observer?.message(message);
    }
  }

  carrierClosed(cause: unknown): void {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    this.#observer?.close(cause);
  }

  close(): Promise<void> {
    if (this.#closePromise === undefined) {
      this.#closed = true;
      if (this.#socket.readyState < WEB_SOCKET_CLOSING) {
        this.#socket.close();
      }
      this.#closePromise = Promise.resolve();
    }
    return this.#closePromise;
  }
}

/**
 * Shared JSON-RPC 2.0 request correlation and bounded lifecycle machinery.
 *
 * This class deliberately does not own carrier framing/reconnect or
 * application notification fanout and result validation.
 */
export class JsonRpcPeer {
  readonly #transport: MessageTransport;
  readonly #options: JsonRpcPeerOptions;
  readonly #pending = new Map<JsonRpcId, PendingRequest>();
  readonly #notificationListeners = new Set<
    (notification: JsonRpcNotification) => void
  >();
  readonly #failureListeners = new Set<(error: Error) => void>();
  readonly #timedOutIds = new Set<JsonRpcId>();
  #unsubscribeTransport: () => void = () => {};
  #pendingBytes = 0;
  #outboundBytes = 0;
  #failure?: Error;
  #closePromise?: Promise<void>;

  constructor(transport: MessageTransport, options: JsonRpcPeerOptions) {
    validateOptions(options);
    this.#transport = transport;
    this.#options = options;
    const unsubscribeTransport = transport.subscribe({
      message: (message) => this.#receive(message),
      close: (cause) => {
        this.#fail(
          new JsonRpcTransportError("JSON-RPC carrier closed", cause),
        );
      },
    });
    this.#unsubscribeTransport = unsubscribeTransport;
    if (this.#failure !== undefined) {
      unsubscribeTransport();
    }
  }

  request<T = unknown>(
    method: string,
    params: object,
    timeoutMs = this.#options.requestTimeoutMs,
  ): Promise<T> {
    if (this.#failure !== undefined) {
      return Promise.reject(this.#failure);
    }
    if (method.length === 0) {
      return Promise.reject(new TypeError("JSON-RPC method must not be empty"));
    }
    if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0) {
      return Promise.reject(new TypeError("JSON-RPC timeout must be a positive integer"));
    }

    const id = this.#options.createRequestId();
    if (
      !isValidId(id) ||
      this.#pending.has(id) ||
      this.#timedOutIds.has(id)
    ) {
      return Promise.reject(new TypeError("JSON-RPC request id must be unique and valid"));
    }

    let message: string;
    try {
      message = JSON.stringify({ jsonrpc: "2.0", id, method, params });
    } catch (cause) {
      const error = new TypeError("JSON-RPC request params are not serializable");
      (error as TypeError & { cause?: unknown }).cause = cause;
      return Promise.reject(error);
    }
    const retainedBytes = messageBytes(message);
    const limit = this.#options.limits;
    if (retainedBytes > limit.maxMessageBytes) {
      return Promise.reject(new JsonRpcLimitError("message_bytes"));
    }
    if (this.#pending.size >= limit.maxPendingRequests) {
      return Promise.reject(new JsonRpcLimitError("pending_count"));
    }
    if (this.#pendingBytes + retainedBytes > limit.maxPendingBytes) {
      return Promise.reject(new JsonRpcLimitError("pending_bytes"));
    }
    if (this.#outboundBytes + retainedBytes > limit.maxOutboundBytes) {
      return Promise.reject(new JsonRpcLimitError("outbound_bytes"));
    }

    let resolveRequest!: (value: T) => void;
    let rejectRequest!: (error: Error) => void;
    const response = new Promise<T>((resolve, reject) => {
      resolveRequest = resolve;
      rejectRequest = reject;
    });
    const timer = setTimeout(() => {
      const pending = this.#pending.get(id);
      if (pending === undefined) {
        return;
      }
      if (
        this.#timedOutIds.size >=
        this.#options.limits.maxPendingRequests
      ) {
        this.#fail(
          new JsonRpcProtocolError(
            "JSON-RPC timeout tombstone capacity was exceeded",
          ),
        );
        return;
      }
      this.#removePending(id, pending);
      this.#timedOutIds.add(id);
      pending.reject(new JsonRpcTimeoutError(method, id));
    }, timeoutMs);
    this.#pending.set(id, {
      retainedBytes,
      timer,
      resolve: (value) => resolveRequest(value as T),
      reject: rejectRequest,
    });
    this.#pendingBytes += retainedBytes;
    this.#outboundBytes += retainedBytes;

    let sending: Promise<void>;
    try {
      sending = this.#transport.send(message);
    } catch (cause) {
      this.#releaseOutbound(retainedBytes);
      this.#fail(
        new JsonRpcTransportError("JSON-RPC carrier rejected a message", cause),
      );
      return response;
    }
    void sending.then(
      () => this.#releaseOutbound(retainedBytes),
      (cause: unknown) => {
        this.#releaseOutbound(retainedBytes);
        this.#fail(
          new JsonRpcTransportError("JSON-RPC carrier rejected a message", cause),
        );
      },
    );
    return response;
  }

  onNotification(
    listener: (notification: JsonRpcNotification) => void,
  ): () => void {
    this.#notificationListeners.add(listener);
    return () => this.#notificationListeners.delete(listener);
  }

  onFailure(listener: (error: Error) => void): () => void {
    if (this.#failure !== undefined) {
      listener(this.#failure);
      return () => {};
    }
    this.#failureListeners.add(listener);
    return () => this.#failureListeners.delete(listener);
  }

  abort(error: Error): Promise<void> {
    this.#fail(error);
    return this.#closeTransport();
  }

  close(): Promise<void> {
    this.#fail(new JsonRpcTransportError("JSON-RPC peer closed"));
    return this.#closeTransport();
  }

  #receive(message: string): void {
    if (this.#failure !== undefined) {
      return;
    }
    let notification: JsonRpcNotification | undefined;
    try {
      if (messageBytes(message) > this.#options.limits.maxMessageBytes) {
        throw new JsonRpcProtocolError(
          "JSON-RPC carrier message exceeds the byte limit",
        );
      }
      const envelope = JSON.parse(message) as unknown;
      if (!isRecord(envelope) || envelope.jsonrpc !== "2.0") {
        throw new JsonRpcProtocolError("Invalid JSON-RPC 2.0 envelope");
      }
      if (hasOwn(envelope, "id")) {
        this.#receiveResponse(envelope);
        return;
      }
      notification = this.#parseNotification(envelope);
    } catch (cause) {
      const error = cause instanceof JsonRpcProtocolError
        ? cause
        : new JsonRpcProtocolError("Invalid JSON-RPC carrier message", cause);
      this.#fail(error);
      return;
    }
    for (const listener of this.#notificationListeners) {
      try {
        listener(notification);
      } catch (cause) {
        const error = cause instanceof Error
          ? cause
          : new JsonRpcProtocolError("JSON-RPC notification listener failed", cause);
        try {
          this.#options.onNotificationError?.(error, notification);
        } catch {
          // The application hook is diagnostic/policy code. Its own failure
          // must not be reclassified as malformed wire input.
        }
      }
    }
  }

  #receiveResponse(envelope: Record<string, unknown>): void {
    const id = envelope.id;
    if (!isValidId(id)) {
      throw new JsonRpcProtocolError("Invalid JSON-RPC response id");
    }
    const hasResult = hasOwn(envelope, "result");
    const hasError = hasOwn(envelope, "error");
    if (hasResult === hasError) {
      throw new JsonRpcProtocolError(
        "JSON-RPC response must contain exactly one result or error",
      );
    }
    if (!hasOnlyKeys(envelope, ["jsonrpc", "id", hasError ? "error" : "result"])) {
      throw new JsonRpcProtocolError("JSON-RPC response contains unknown fields");
    }

    let remoteError: JsonRpcRemoteError | undefined;
    if (hasError) {
      const remote = envelope.error;
      if (!isRecord(remote)) {
        throw new JsonRpcProtocolError("Invalid JSON-RPC error envelope");
      }
      const allowed = hasOwn(remote, "data")
        ? ["code", "message", "data"]
        : ["code", "message"];
      if (
        !hasOnlyKeys(remote, allowed) ||
        !Number.isSafeInteger(remote.code) ||
        typeof remote.message !== "string"
      ) {
        throw new JsonRpcProtocolError("Invalid JSON-RPC error envelope");
      }
      remoteError = new JsonRpcRemoteError(
        remote.code as number,
        remote.message,
        remote.data,
      );
    }

    if (this.#timedOutIds.delete(id)) {
      return;
    }
    const pending = this.#pending.get(id);
    if (pending === undefined) {
      throw new JsonRpcProtocolError("Unknown JSON-RPC response id");
    }
    this.#removePending(id, pending);
    if (remoteError !== undefined) {
      pending.reject(remoteError);
    } else {
      pending.resolve(envelope.result);
    }
  }

  #parseNotification(envelope: Record<string, unknown>): JsonRpcNotification {
    if (
      typeof envelope.method !== "string" ||
      envelope.method.length === 0 ||
      !hasOnlyKeys(
        envelope,
        hasOwn(envelope, "params")
          ? ["jsonrpc", "method", "params"]
          : ["jsonrpc", "method"],
      )
    ) {
      throw new JsonRpcProtocolError("Invalid JSON-RPC notification envelope");
    }
    return {
      method: envelope.method,
      params: envelope.params,
    };
  }

  #removePending(id: JsonRpcId, pending: PendingRequest): void {
    clearTimeout(pending.timer);
    this.#pending.delete(id);
    this.#pendingBytes -= pending.retainedBytes;
  }

  #releaseOutbound(bytes: number): void {
    this.#outboundBytes = Math.max(0, this.#outboundBytes - bytes);
  }

  #fail(error: Error): void {
    if (this.#failure !== undefined) {
      return;
    }
    this.#failure = error;
    this.#unsubscribeTransport();
    for (const [id, pending] of this.#pending) {
      this.#removePending(id, pending);
      pending.reject(error);
    }
    for (const listener of this.#failureListeners) {
      try {
        listener(error);
      } catch {
        // Failure listeners are observers. Resource reclamation and the other
        // observers must still run if application diagnostics throw.
      }
    }
    this.#failureListeners.clear();
    this.#notificationListeners.clear();
    void this.#closeTransport().catch(() => {});
  }

  #closeTransport(): Promise<void> {
    if (this.#closePromise === undefined) {
      try {
        this.#closePromise = Promise.resolve(this.#transport.close());
      } catch (error) {
        this.#closePromise = Promise.reject(error);
      }
    }
    return this.#closePromise;
  }
}

function validateOptions(options: JsonRpcPeerOptions): void {
  if (!Number.isSafeInteger(options.requestTimeoutMs) || options.requestTimeoutMs <= 0) {
    throw new TypeError("JSON-RPC request timeout must be a positive integer");
  }
  for (const [name, value] of Object.entries(options.limits)) {
    if (!Number.isSafeInteger(value) || value <= 0) {
      throw new TypeError(`JSON-RPC ${name} must be a positive integer`);
    }
  }
}

function assertPositiveInteger(value: number, name: string): void {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new TypeError(`${name} must be a positive integer`);
  }
}

async function waitForWebSocketDrain(
  socket: WebSocketLike,
  deadline: number,
): Promise<void> {
  while (socket.bufferedAmount > 0) {
    if (socket.readyState !== WEB_SOCKET_OPEN) {
      throw new Error("WebSocket closed while a message was backpressured");
    }
    if (Date.now() >= deadline) {
      throw new Error("WebSocket send buffer drain deadline was exceeded");
    }
    await new Promise((resolve) => setTimeout(resolve, 4));
  }
}

function isValidId(value: unknown): value is JsonRpcId {
  return (
    (typeof value === "number" && Number.isSafeInteger(value) && value >= 0) ||
    (typeof value === "string" && value.length > 0)
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOnlyKeys(
  record: Record<string, unknown>,
  keys: readonly string[],
): boolean {
  const actual = Object.keys(record);
  return actual.length === keys.length && actual.every((key) => keys.includes(key));
}

function hasOwn(record: Record<string, unknown>, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(record, key);
}

function messageBytes(message: string): number {
  return UTF8.encode(message).byteLength;
}
