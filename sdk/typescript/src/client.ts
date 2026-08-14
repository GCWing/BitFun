import type { InitializeResult, QueryStartParams, QueryStartResult } from "./internal/wire/index.js";
import type { JsonRpcConnection } from "./internal/json-rpc.js";
import { Query } from "./query.js";
import { Session, Sessions } from "./session.js";
import type { AgentCapabilities, AgentClientOptions, QueryInput } from "./types.js";

export class AgentClient {
  readonly #connection: JsonRpcConnection;
  readonly #options: AgentClientOptions;
  readonly capabilities: AgentCapabilities;
  readonly sessions: Sessions;
  readonly #queries = new Set<Query>();
  readonly #ownedSessions = new Set<Session>();
  #closed = false;
  #closePromise?: Promise<void>;

  static async start(options: AgentClientOptions): Promise<AgentClient> {
    const hostPath = options.hostPath ?? process.env.BITFUN_SDK_HOST_PATH;
    if (hostPath === undefined || hostPath.length === 0) {
      const { SdkError } = await import("./errors.js");
      throw new SdkError("SDK Host executable is unavailable", {
        code: "not_found",
        stage: "initialize",
        retryable: false,
        correlationId: "local:host_start",
        outcomeCertainty: "not_started",
      });
    }
    const [{ createAgentClient }, { startManagedHost }] = await Promise.all([
      import("./internal/client.js"),
      import("./internal/managed-host.js"),
    ]);
    const transport = await startManagedHost({
      executable: hostPath,
      cwd: options.cwd,
    });
    try {
      return await createAgentClient(transport, options);
    } catch (error) {
      await transport.close();
      throw error;
    }
  }

  private constructor(
    connection: JsonRpcConnection,
    options: AgentClientOptions,
    initialized: InitializeResult,
  ) {
    this.#connection = connection;
    this.#options = options;
    this.capabilities = Object.freeze({
      query: initialized.capabilities.query,
      sessions: initialized.capabilities.sessionCreate,
      cancellation: initialized.capabilities.queryCancel,
    });
    this.sessions = Sessions.forClient(
      connection,
      options.cwd,
      (query) => this.#trackQuery(query),
      (session) => this.#trackSession(session),
      () => this.#ensureOpen(),
    );
  }

  /** @internal */
  static create(
    connection: JsonRpcConnection,
    options: AgentClientOptions,
    initialized: InitializeResult,
  ): AgentClient {
    return new AgentClient(connection, options, initialized);
  }

  async query(input: QueryInput): Promise<Query> {
    this.#ensureOpen();
    const params: QueryStartParams = {
      prompt: input.prompt,
      sessionId: null,
      sessionName: null,
      agent: input.agent ?? null,
      cwd: this.#options.cwd,
      model: input.model ?? null,
    };
    const started = await this.#connection.request<QueryStartResult>(
      "query/start",
      params,
    );
    if (!started.accepted) {
      throw new Error("SDK Host did not accept the Query");
    }
    return this.#trackQuery(Query.create(this.#connection, started));
  }

  close(): Promise<void> {
    this.#closePromise ??= this.#closeOwnedResources();
    return this.#closePromise;
  }

  async #closeOwnedResources(): Promise<void> {
    this.#closed = true;
    const querySettlements = await Promise.allSettled(
      [...this.#queries].map((query) => query.close()),
    );
    const sessionSettlements = await Promise.allSettled(
      [...this.#ownedSessions].map((session) => session.close()),
    );
    let shutdownError: unknown;
    try {
      await this.#connection.shutdown();
    } catch (error) {
      shutdownError = error;
    }
    const cleanupFailure = [...querySettlements, ...sessionSettlements].find(
      (settlement): settlement is PromiseRejectedResult =>
        settlement.status === "rejected",
    );
    if (cleanupFailure !== undefined) {
      throw cleanupFailure.reason;
    }
    if (shutdownError !== undefined) {
      throw shutdownError;
    }
  }

  [Symbol.asyncDispose](): Promise<void> {
    return this.close();
  }

  #ensureOpen(): void {
    if (this.#closed) {
      throw new Error("AgentClient is closed");
    }
  }

  #trackQuery(query: Query): Query {
    this.#queries.add(query);
    query.onClosed(() => this.#queries.delete(query));
    void query.result().then(
      () => {
        if (!query.ownsSession) {
          this.#queries.delete(query);
        }
      },
      () => this.#queries.delete(query),
    );
    return query;
  }

  #trackSession(session: Session): void {
    this.#ownedSessions.add(session);
    session.onClosed(() => this.#ownedSessions.delete(session));
  }
}
