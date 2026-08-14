import { createInterface } from "node:readline";

const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
for await (const line of lines) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    write({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: 1,
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
