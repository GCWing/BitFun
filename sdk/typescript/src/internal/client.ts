import { AgentClient } from "../client.js";
import type { AgentClientOptions } from "../types.js";
import { JsonRpcConnection } from "./json-rpc.js";
import type { HostTransport } from "./transport.js";
import type { InitializeParams, InitializeResult } from "./wire/index.js";

const PROTOCOL_VERSION = 6;
const DEFAULT_INITIALIZE_TIMEOUT_MS = 30_000;

export async function createAgentClient(
  transport: HostTransport,
  options: Pick<AgentClientOptions, "cwd" | "initializeTimeoutMs" | "model">,
): Promise<AgentClient> {
  const connection = new JsonRpcConnection(transport);
  const params: InitializeParams = {
    protocolVersion: PROTOCOL_VERSION,
    clientInfo: { name: "@openbitfun/agent-sdk", version: "0.0.0" },
    capabilities: {
      serverNotifications: true,
      permissionResponses: true,
    },
    model: {
      provider: options.model.provider,
      model: options.model.model,
      apiKey: options.model.apiKey,
      baseUrl: options.model.baseUrl,
    },
  };
  const initialized = await connection.request<InitializeResult>(
    "initialize",
    params,
    options.initializeTimeoutMs ?? DEFAULT_INITIALIZE_TIMEOUT_MS,
  );
  if (initialized.protocolVersion !== PROTOCOL_VERSION) {
    await connection.shutdown();
    throw new Error(
      `SDK Host selected protocol ${initialized.protocolVersion}; expected ${PROTOCOL_VERSION}`,
    );
  }
  return AgentClient.create(connection, options, initialized);
}
