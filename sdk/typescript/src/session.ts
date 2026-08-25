import type {
  QueryStartParams,
  QueryStartResult,
  SessionCloseParams,
  SessionCreateParams,
  SessionCreateResult,
  SessionResumeParams,
} from "./internal/wire/index.js";
import type { JsonRpcConnection } from "./internal/json-rpc.js";
import { normalizeInput } from "./internal/input.js";
import { withTimeout } from "./internal/deadline.js";
import { isConnectionUnusableError, SdkError } from "./errors.js";
import { Query } from "./query.js";
import type { SessionCreateInput, SessionLifetime, TurnInput } from "./types.js";

export class Sessions {
  readonly #connection: JsonRpcConnection;
  readonly #cwd: string;
  readonly #modelId: string;
  readonly #onQuery: (query: Query) => Query;
  readonly #onSession: (session: Session) => void;
  readonly #ensureClientOpen: () => void;

  /** @internal */
  static forClient(
    connection: JsonRpcConnection,
    cwd: string,
    modelId: string,
    onQuery: (query: Query) => Query,
    onSession: (session: Session) => void,
    ensureClientOpen: () => void,
  ): Sessions {
    return new Sessions(
      connection,
      cwd,
      modelId,
      onQuery,
      onSession,
      ensureClientOpen,
    );
  }

  private constructor(
    connection: JsonRpcConnection,
    cwd: string,
    modelId: string,
    onQuery: (query: Query) => Query,
    onSession: (session: Session) => void,
    ensureClientOpen: () => void,
  ) {
    this.#connection = connection;
    this.#cwd = cwd;
    this.#modelId = modelId;
    this.#onQuery = onQuery;
    this.#onSession = onSession;
    this.#ensureClientOpen = ensureClientOpen;
  }

  async create(input: SessionCreateInput = {}): Promise<Session> {
    this.#ensureClientOpen();
    const params: SessionCreateParams = {
      sessionName: input.sessionName ?? null,
      agent: input.agent ?? null,
      cwd: this.#cwd,
      model: this.#modelId,
    };
    const created = await this.#connection.request<SessionCreateResult>(
      "session/create",
      params,
    );
    const session = Session.create(this.#connection, created, this.#onQuery);
    this.#onSession(session);
    return session;
  }

  async resume(sessionId: string): Promise<Session> {
    this.#ensureClientOpen();
    if (sessionId.trim().length === 0) {
      throw new Error("sessionId must not be empty");
    }
    const params: SessionResumeParams = { sessionId };
    const resumed = await this.#connection.request<SessionCreateResult>(
      "session/resume",
      params,
    );
    const session = Session.create(this.#connection, resumed, this.#onQuery);
    this.#onSession(session);
    return session;
  }
}

export class Session {
  readonly #connection: JsonRpcConnection;
  readonly id: string;
  readonly name: string;
  readonly agent: string;
  readonly lifetime: SessionLifetime;
  readonly workspacePath?: string;
  readonly #onQuery: (query: Query) => Query;
  readonly #closeTimeoutMs: number;
  readonly #closedHandlers = new Set<() => void>();
  #closed = false;
  #closePromise?: Promise<void>;

  /** @internal */
  static create(
    connection: JsonRpcConnection,
    created: SessionCreateResult,
    onQuery: (query: Query) => Query,
    closeTimeoutMs = 7_000,
  ): Session {
    return new Session(connection, created, onQuery, closeTimeoutMs);
  }

  private constructor(
    connection: JsonRpcConnection,
    created: SessionCreateResult,
    onQuery: (query: Query) => Query,
    closeTimeoutMs: number,
  ) {
    this.#connection = connection;
    this.id = created.sessionId;
    this.name = created.sessionName;
    this.agent = created.agent;
    this.lifetime = created.lifetime;
    this.workspacePath = created.workspacePath;
    this.#onQuery = onQuery;
    this.#closeTimeoutMs = closeTimeoutMs;
  }

  async startTurn(input: TurnInput): Promise<Query> {
    this.#ensureOpen();
    const normalized = normalizeInput(input.prompt);
    const params: QueryStartParams = {
      prompt: normalized.prompt,
      images: normalized.images,
      outputSchema: input.outputSchema,
      sessionId: this.id,
    };
    const started = await this.#connection.request<QueryStartResult>(
      "query/start",
      params,
    );
    if (!started.accepted || started.sessionId !== this.id) {
      throw new Error("SDK Host did not accept the Turn for this Session");
    }
    return this.#onQuery(Query.create(this.#connection, started));
  }

  close(): Promise<void> {
    this.#closePromise ??= this.#closeSession();
    return this.#closePromise;
  }

  /** @internal */
  onClosed(handler: () => void): void {
    this.#closedHandlers.add(handler);
  }

  async #closeSession(): Promise<void> {
    this.#closed = true;
    const params: SessionCloseParams = { sessionId: this.id };
    const timeoutError = new SdkError(
      "SDK Host did not close Session before its cleanup deadline",
      {
        code: "cleanup_required",
        stage: "session",
        retryable: false,
        correlationId: `session:${this.id}`,
        outcomeCertainty: "unknown",
        recovery: "restart_host",
      },
    );
    try {
      await withTimeout(
        this.#connection.request("session/close", params),
        this.#closeTimeoutMs,
        () => timeoutError,
      );
    } catch (error) {
      if (isConnectionUnusableError(error)) {
        await this.#connection.abort(error);
      }
      throw error;
    } finally {
      for (const handler of this.#closedHandlers) {
        handler();
      }
      this.#closedHandlers.clear();
    }
  }

  [Symbol.asyncDispose](): Promise<void> {
    return this.close();
  }

  #ensureOpen(): void {
    if (this.#closed) {
      throw new Error("Session is closed");
    }
  }
}
