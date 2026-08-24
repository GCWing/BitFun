import { createInterface } from "node:readline";

const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
for await (const line of lines) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    if (
      request.params?.protocolVersion !== 2 ||
      request.params?.model?.apiKey !== "fixture-secret"
    ) {
      throw new Error("Invalid initialize request");
    }
    write({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: 2,
        runtimeVersion: "fixture",
        stability: "not_delivered",
        capabilities: {
          sessionCreate: true,
          sessionCreateLifetime: "connection",
          query: true,
          queryCancel: true,
          sessionClose: true,
          eventStream: true,
          structuredOutput: false,
          usage: false,
          customTools: false,
          permissionCallbacks: false,
          hooks: false,
          mcpConfiguration: false,
          prestartedTransport: false,
        },
        modelId: "sdk:openai:resolved",
      },
    });
    continue;
  }
  if (request.method === "shutdown") {
    write({ jsonrpc: "2.0", id: request.id, result: { accepted: true } });
    break;
  }
  throw new Error(`Unexpected method: ${request.method}`);
}

function write(value) {
  process.stdout.write(`${JSON.stringify(value)}\n`);
}
