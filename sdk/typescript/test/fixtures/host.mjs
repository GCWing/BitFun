import { createInterface } from "node:readline";

const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
for await (const line of lines) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    if (
      request.params?.protocolVersion !== 6 ||
      request.params?.model?.apiKey !== "fixture-secret"
    ) {
      throw new Error("Invalid initialize request");
    }
    write({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: 6,
        runtimeVersion: "fixture",
        stability: "not_delivered",
        capabilities: {
          sessionCreate: true,
          sessionCreateLifetime: "durable",
          sessionResume: true,
          query: true,
          queryCancel: true,
          sessionClose: true,
          eventStream: true,
          toolEvents: true,
          imageInput: true,
          structuredOutput: true,
          usage: true,
          customTools: false,
          permissionResponses: true,
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
