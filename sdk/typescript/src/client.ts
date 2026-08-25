import type { InitializeResult, QueryStartParams, QueryStartResult } from "./internal/wire/index.js";
import type { JsonRpcConnection } from "./internal/json-rpc.js";
import { resolveHostPath } from "./internal/host-path.js";
import { normalizeInput } from "./internal/input.js";
import { SdkError } from "./errors.js";
import { Query } from "./query.js";
import { Session, Sessions } from "./session.js";
import type {
  AgentCapabilities,
  AgentClientOptions,
  AgentModelOptions,
  QueryInput,
} from "./types.js";

const SUPPORTED_MODEL_PROVIDERS = new Set([
  "openai",
  "responses",
  "anthropic",
  "gemini",
]);

export class AgentClient {
  readonly #connection: JsonRpcConnection;
  readonly #cwd: string;
  readonly #modelId: string;
  readonly capabilities: AgentCapabilities;
  readonly sessions: Sessions;
  readonly #queries = new Set<Query>();
  readonly #ownedSessions = new Set<Session>();
  #closed = false;
  #closePromise?: Promise<void>;

  static async start(options: AgentClientOptions): Promise<AgentClient> {
    validateModelOptions(options.model as unknown);
    const hostPath = resolveHostPath(options.hostPath);
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
    options: Pick<AgentClientOptions, "cwd">,
    initialized: InitializeResult,
  ) {
    this.#connection = connection;
    this.#cwd = options.cwd;
    this.#modelId = initialized.modelId;
    this.capabilities = Object.freeze({
      query: initialized.capabilities.query,
      sessions: initialized.capabilities.sessionCreate,
      cancellation: initialized.capabilities.queryCancel,
      eventStream: initialized.capabilities.eventStream,
      toolEvents: initialized.capabilities.toolEvents,
      imageInput: initialized.capabilities.imageInput,
      permissionResponses: initialized.capabilities.permissionResponses,
      structuredOutput: initialized.capabilities.structuredOutput,
      usage: initialized.capabilities.usage,
      customTools: initialized.capabilities.customTools,
      hooks: initialized.capabilities.hooks,
      mcpConfiguration: initialized.capabilities.mcpConfiguration,
    });
    this.sessions = Sessions.forClient(
      connection,
      this.#cwd,
      this.#modelId,
      (query) => this.#trackQuery(query),
      (session) => this.#trackSession(session),
      () => this.#ensureOpen(),
    );
  }

  /** @internal */
  static create(
    connection: JsonRpcConnection,
    options: Pick<AgentClientOptions, "cwd">,
    initialized: InitializeResult,
  ): AgentClient {
    return new AgentClient(connection, options, initialized);
  }

  async query(input: QueryInput): Promise<Query> {
    this.#ensureOpen();
    const normalized = normalizeInput(input.prompt);
    const params: QueryStartParams = {
      prompt: normalized.prompt,
      images: normalized.images,
      outputSchema: input.outputSchema,
      sessionId: null,
      sessionName: null,
      agent: input.agent ?? null,
      cwd: this.#cwd,
      model: this.#modelId,
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

function validateModelOptions(value: unknown): asserts value is AgentModelOptions {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    invalidModel("model is required");
  }
  const model = value as Record<string, unknown>;
  if (
    typeof model.provider !== "string" ||
    !SUPPORTED_MODEL_PROVIDERS.has(model.provider)
  ) {
    invalidModel("model.provider is unsupported");
  }
  if (typeof model.model !== "string" || model.model.trim().length === 0) {
    invalidModel("model.model is required");
  }
  if (typeof model.apiKey !== "string" || model.apiKey.trim().length === 0) {
    invalidModel("model.apiKey is required");
  }
  if (model.baseUrl === undefined) {
    return;
  }
  const invalidBaseUrl =
    "model.baseUrl must be an absolute http or https URL without credentials, query, or fragment";
  if (typeof model.baseUrl !== "string") {
    invalidModel(invalidBaseUrl);
  }
  let url: URL;
  try {
    url = new URL(model.baseUrl);
  } catch {
    invalidModel(invalidBaseUrl);
  }
  if (
    (url.protocol !== "http:" && url.protocol !== "https:") ||
    url.hostname.length === 0 ||
    url.username.length > 0 ||
    url.password.length > 0 ||
    url.search.length > 0 ||
    url.hash.length > 0
  ) {
    invalidModel(invalidBaseUrl);
  }
}

function invalidModel(message: string): never {
  throw new SdkError(message, {
    code: "invalid_request",
    stage: "initialize",
    retryable: false,
    correlationId: "local:model_validation",
    outcomeCertainty: "not_started",
  });
}
