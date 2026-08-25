import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import ts from "typescript";

const scriptsDirectory = dirname(fileURLToPath(import.meta.url));
const packageRoot = resolve(scriptsDirectory, "..");
const wireDirectory = join(packageRoot, "src", "internal", "wire");
const validatorPath = join(packageRoot, "src", "internal", "wire-validators.ts");

test("Rust wire export produces executable validators for every type", async () => {
  const typeNames = (await readdir(wireDirectory))
    .filter((file) => file.endsWith(".ts") && file !== "index.ts")
    .map((file) => file.slice(0, -3))
    .sort();
  const validatorSource = await readFile(validatorPath, "utf8").catch((error) => {
    if (error?.code === "ENOENT") {
      throw new Error(
        "generated wire validators are missing; run the SDK build before this test",
      );
    }
    throw error;
  });
  const javascript = ts.transpileModule(validatorSource, {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
  }).outputText;
  const validators = await import(
    `data:text/javascript;base64,${Buffer.from(javascript).toString("base64")}`
  );

  for (const typeName of typeNames) {
    assert.equal(
      typeof validators[`is${typeName}`],
      "function",
      `${typeName} must have an executable runtime validator`,
    );
  }

  const initializeResult = {
    protocolVersion: 6,
    runtimeVersion: "0.1.0",
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
  };
  assert.equal(validators.isInitializeResult(initializeResult), true);
  const { modelId: _modelId, ...initializeResultWithoutModel } = initializeResult;
  assert.equal(validators.isInitializeResult(initializeResultWithoutModel), false);
  assert.equal(
    validators.isInitializeResult({ ...initializeResult, unexpected: true }),
    false,
  );
  assert.equal(
    validators.isInitializeResult({
      ...initializeResult,
      capabilities: {
        ...initializeResult.capabilities,
        eventStream: "yes",
      },
    }),
    false,
  );

  assert.equal(
    validators.isQueryEventParams({
      queryId: "query_1",
      sessionId: "session_1",
      turnId: "turn_1",
      operationId: "operation_1",
      sequence: 1,
      event: { type: "assistant_text_delta", text: "hello" },
    }),
    true,
  );
  assert.equal(
    validators.isQueryEventParams({
      queryId: "query_1",
      sessionId: "session_1",
      turnId: "turn_1",
      operationId: "operation_1",
      sequence: 1,
    }),
    false,
  );

  assert.equal(
    validators.isSessionCreateResult({
      sessionId: "session_1",
      sessionName: "Main",
      agent: "agentic",
      lifetime: "connection",
      executionTarget: {
        kind: "managedWorktree",
        worktreeId: "worktree_1",
        rootPath: "D:/workspace/worktrees/worktree_1",
        lifecycle: "managed",
      },
    }),
    true,
  );
  assert.equal(
    validators.isSessionCreateResult({
      sessionId: "session_1",
      sessionName: "Main",
      agent: "agentic",
      lifetime: "connection",
      executionTarget: {
        kind: "managedWorktree",
        rootPath: "D:/workspace/worktrees/worktree_1",
        lifecycle: "temporary",
      },
    }),
    false,
  );

  assert.equal(validators.isShutdownParams({}), true);
  assert.equal(validators.isShutdownParams({ reason: "later" }), false);
});
