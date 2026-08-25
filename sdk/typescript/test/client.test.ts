import assert from "node:assert/strict";
import { PassThrough } from "node:stream";
import { createInterface } from "node:readline";
import test from "node:test";

import { AgentClient, SdkError } from "../src/index.js";
import { createAgentClient } from "../src/internal/client.js";
import type {
  AgentClientOptions,
  Input,
  QueryInput,
  SessionCreateInput,
  UserInput,
} from "../src/types.js";

const clientOptions = {
  cwd: "D:/workspace/project",
  hostPath: process.execPath,
  model: {
    provider: "openai" as const,
    model: "fixture-model",
    apiKey: "fixture-secret",
    baseUrl: "http://127.0.0.1:43123/v1",
  },
} satisfies AgentClientOptions;

const packageHostOptions = {
  cwd: "D:/workspace/project",
  model: clientOptions.model,
} satisfies AgentClientOptions;

// @ts-expect-error Query model selection is bound at AgentClient.start.
const queryModelOverride: QueryInput = { prompt: "hello", model: "attempted-override" };
// @ts-expect-error Session model selection is bound at AgentClient.start.
const sessionModelOverride: SessionCreateInput = { model: "attempted-override" };
void queryModelOverride;
void sessionModelOverride;
void packageHostOptions;

const multimodalInput: Input = [
  { type: "text", text: "hello" },
  { type: "local_image", path: "screenshots/fixture.png" },
] satisfies UserInput[];
void multimodalInput;

test("a Query streams tool and permission events before the terminal Result", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  const initializeRequests: unknown[] = [];
  const host = runFixtureHost(clientToHost, hostToClient, initializeRequests);
  const client = await createAgentClient(
    {
      readable: hostToClient,
      writable: clientToHost,
      close: async () => {
        clientToHost.end();
        await host;
      },
    },
    clientOptions,
  );

  assert.ok(client instanceof AgentClient);
  assert.equal(initializeRequests.length, 1);
  assert.deepEqual(initializeRequests[0], {
    protocolVersion: 6,
    clientInfo: { name: "@bitfun/agent-sdk", version: "0.0.0" },
    capabilities: {
      serverNotifications: true,
      permissionResponses: true,
    },
    model: {
      provider: "openai",
      model: "fixture-model",
      apiKey: "fixture-secret",
      baseUrl: "http://127.0.0.1:43123/v1",
    },
  });
  const query = await client.query({
    prompt: [
      { type: "text", text: "hello" },
      { type: "local_image", path: "screenshots/fixture.png" },
      { type: "text", text: "focus on the layout" },
    ],
    outputSchema: {
      type: "object",
      properties: { summary: { type: "string" } },
      required: ["summary"],
    },
    model: "attempted-override",
  } as QueryInput);
  assert.equal(query.id, "query-1");
  assert.equal(query.operationId, "operation-1");
  assert.deepEqual(query.turn, { id: "turn-1", sessionId: "session-1" });
  assert.deepEqual(client.capabilities, {
    query: true,
    sessions: true,
    cancellation: true,
    eventStream: true,
    toolEvents: true,
    imageInput: true,
    permissionResponses: true,
    structuredOutput: true,
    usage: true,
    customTools: false,
    hooks: false,
    mcpConfiguration: false,
  });
  const items = [];
  for await (const item of query) {
    items.push(item);
    if (item.type === "permission_request") {
      await query.respondPermission(item.requestId, { decision: "allow_once" });
      await assert.rejects(
        query.respondPermission(item.requestId, { decision: "allow_once" }),
        /unknown, expired, or already answered/,
      );
    }
  }

  assert.deepEqual(items, [
    {
      type: "tool_event",
      queryId: "query-1",
      sessionId: "session-1",
      turnId: "turn-1",
      operationId: "operation-1",
      sequence: 1,
      toolCallId: "tool-1",
      toolName: "Read",
      status: "started",
    },
    {
      type: "permission_request",
      queryId: "query-1",
      sessionId: "session-1",
      turnId: "turn-1",
      operationId: "operation-1",
      sequence: 2,
      requestId: "permission-1",
      action: "read",
      resources: ["README.md"],
      source: { kind: "tool_call", identity: "Read" },
      toolCallId: "tool-1",
      responseTimeoutMs: 120_000,
    },
    {
      type: "tool_event",
      queryId: "query-1",
      sessionId: "session-1",
      turnId: "turn-1",
      operationId: "operation-1",
      sequence: 3,
      toolCallId: "tool-1",
      toolName: "Read",
      status: "completed",
      durationMs: 12,
    },
    {
      type: "assistant_text_delta",
      queryId: "query-1",
      sessionId: "session-1",
      turnId: "turn-1",
      operationId: "operation-1",
      sequence: 4,
      text: "fixture result",
    },
    {
      type: "result",
      queryId: "query-1",
      sessionId: "session-1",
      turnId: "turn-1",
      operationId: "operation-1",
      status: "completed",
      outputText: "fixture result",
      structuredOutput: { summary: "fixture result" },
      usage: {
        inputTokens: 100,
        outputTokens: 25,
        totalTokens: 125,
        cachedTokens: 40,
      },
    },
  ]);
  assert.equal((await query.result()).outputText, "fixture result");
  assert.equal(typeof query[Symbol.asyncDispose], "function");

  await client.close();
  assert.equal(typeof client[Symbol.asyncDispose], "function");
});

test("an explicit Session starts Turns on the existing client connection", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  const methods: string[] = [];
  const host = runSessionFixtureHost(clientToHost, hostToClient, methods);
  const client = await createAgentClient(
    {
      readable: hostToClient,
      writable: clientToHost,
      close: async () => {
        clientToHost.end();
        await host;
      },
    },
    clientOptions,
  );

  const session = await client.sessions.create({
    agent: "agentic",
    model: "attempted-override",
  } as SessionCreateInput);
  assert.equal(session.id, "session-explicit");
  assert.equal(session.agent, "agentic");
  assert.equal(session.lifetime, "durable");

  const query = await session.startTurn({
    prompt: [{ type: "local_image", path: "screenshots/continued.webp" }],
    outputSchema: { type: "object" },
  });
  assert.equal((await query.result()).outputText, "continued");
  await session.close();
  assert.equal(typeof session[Symbol.asyncDispose], "function");
  await client.close();

  assert.deepEqual(methods, [
    "initialize",
    "session/create",
    "query/start",
    "session/close",
    "shutdown",
  ]);
});

test("a durable Session resumes on a new client connection", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  const methods: string[] = [];
  const host = runResumeFixtureHost(clientToHost, hostToClient, methods);
  const client = await createAgentClient(
    {
      readable: hostToClient,
      writable: clientToHost,
      close: async () => {
        clientToHost.end();
        await host;
      },
    },
    clientOptions,
  );

  const session = await client.sessions.resume("session-persisted");
  assert.equal(session.id, "session-persisted");
  assert.equal(session.lifetime, "durable");
  await session.close();
  await client.close();

  assert.deepEqual(methods, [
    "initialize",
    "session/resume",
    "session/close",
    "shutdown",
  ]);
});

test("Query cancel and close are idempotent and the Host Result remains authoritative", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  let cancelRequests = 0;
  const host = runCancelFixtureHost(clientToHost, hostToClient, () => {
    cancelRequests += 1;
  });
  const client = await createAgentClient(
    {
      readable: hostToClient,
      writable: clientToHost,
      close: async () => {
        clientToHost.end();
        await host;
      },
    },
    clientOptions,
  );

  const query = await client.query({ prompt: "wait" });
  await Promise.all([query.cancel(), query.cancel()]);
  const result = await query.result();
  assert.equal(result.status, "cancelled");
  assert.equal(result.error?.code, "cancelled");
  await query.close();
  assert.equal(cancelRequests, 1);

  await client.close();
});

test("leaving Query iteration early cancels and settles the Turn", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  let cancelRequests = 0;
  const host = runCancelFixtureHost(
    clientToHost,
    hostToClient,
    () => {
      cancelRequests += 1;
    },
    true,
  );
  const client = await createAgentClient(
    {
      readable: hostToClient,
      writable: clientToHost,
      close: async () => {
        clientToHost.end();
        await host;
      },
    },
    clientOptions,
  );

  const query = await client.query({ prompt: "stream" });
  for await (const item of query) {
    assert.equal(item.type, "assistant_text_delta");
    break;
  }
  assert.equal(cancelRequests, 1);
  assert.equal((await query.result()).status, "cancelled");

  await client.close();
});

test("Host loss rejects an accepted Query with unknown outcome instead of fabricating a Result", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  let transportClosed = false;
  const host = runProcessLossFixtureHost(clientToHost, hostToClient);
  const client = await createAgentClient(
    {
      readable: hostToClient,
      writable: clientToHost,
      close: async () => {
        transportClosed = true;
        clientToHost.end();
        await host;
      },
    },
    clientOptions,
  );

  const query = await client.query({ prompt: "may have side effects" });
  await assert.rejects(query.result(), (error: unknown) => {
    assert.ok(error instanceof SdkError);
    assert.equal(error.code, "process_lost");
    assert.equal(error.stage, "protocol");
    assert.equal(error.outcomeCertainty, "unknown");
    return true;
  });
  await assert.rejects(query[Symbol.asyncIterator]().next(), SdkError);

  await client.close();
  assert.equal(transportClosed, true);
});

test("an empty initialized model id fails the connection closed", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  let transportClosed = false;
  const host = runEmptyModelIdFixtureHost(clientToHost, hostToClient);

  await assert.rejects(
    createAgentClient(
      {
        readable: hostToClient,
        writable: clientToHost,
        close: async () => {
          transportClosed = true;
          clientToHost.end();
          await host;
        },
      },
      clientOptions,
    ),
    (error: unknown) => {
      assert.ok(error instanceof SdkError);
      assert.equal(error.code, "process_lost");
      assert.equal(error.stage, "protocol");
      assert.equal(error.outcomeCertainty, "unknown");
      return true;
    },
  );
  assert.equal(transportClosed, true);
});

test("AgentClient.close settles owned Queries before shutting down its connection", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  const methods: string[] = [];
  const host = runClientCloseFixtureHost(clientToHost, hostToClient, methods);
  const client = await createAgentClient(
    {
      readable: hostToClient,
      writable: clientToHost,
      close: async () => {
        clientToHost.end();
        await host;
      },
    },
    clientOptions,
  );

  const query = await client.query({ prompt: "still running" });
  await client.close();
  assert.equal((await query.result()).status, "cancelled");
  assert.deepEqual(methods, [
    "initialize",
    "query/start",
    "query/cancel",
    "session/close",
    "shutdown",
  ]);
});

test("Host operation errors preserve stable SDK error facts", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  const host = runOperationErrorFixtureHost(clientToHost, hostToClient);
  const client = await createAgentClient(
    {
      readable: hostToClient,
      writable: clientToHost,
      close: async () => {
        clientToHost.end();
        await host;
      },
    },
    clientOptions,
  );

  await assert.rejects(client.query({ prompt: "requires auth" }), (error: unknown) => {
    assert.ok(error instanceof SdkError);
    assert.equal(error.code, "action_required");
    assert.equal(error.stage, "query");
    assert.equal(error.retryable, false);
    assert.equal(error.correlationId, "request:query-start");
    assert.equal(error.outcomeCertainty, "not_started");
    assert.equal(error.recovery, "initialize");
    return true;
  });

  await client.close();
});

test("unknown Host error facts fail the protocol closed", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  const host = runInvalidErrorFixtureHost(clientToHost, hostToClient);
  const client = await createAgentClient(
    {
      readable: hostToClient,
      writable: clientToHost,
      close: async () => {
        clientToHost.end();
        await host;
      },
    },
    clientOptions,
  );

  await assert.rejects(client.query({ prompt: "invalid error" }), (error: unknown) => {
    assert.ok(error instanceof SdkError);
    assert.equal(error.code, "process_lost");
    assert.equal(error.outcomeCertainty, "unknown");
    return true;
  });
  await client.close();
});

test("ambiguous JSON-RPC response envelopes fail the protocol closed", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  const host = runAmbiguousResponseFixtureHost(clientToHost, hostToClient);
  const client = await createAgentClient(
    {
      readable: hostToClient,
      writable: clientToHost,
      close: async () => {
        clientToHost.end();
        await host;
      },
    },
    clientOptions,
  );

  await assert.rejects(client.query({ prompt: "reject ambiguous response" }), (error: unknown) => {
    assert.ok(error instanceof SdkError);
    assert.equal(error.code, "process_lost");
    assert.equal(error.outcomeCertainty, "unknown");
    return true;
  });
  await client.close();
});

test("unknown Query event and Result status fail the protocol closed", async (context) => {
  const cases = [
    {
      name: "unknown event",
      notification: {
        jsonrpc: "2.0",
        method: "query/event",
        params: {
          queryId: "query-invalid",
          sessionId: "session-invalid",
          turnId: "turn-invalid",
          operationId: "operation-invalid",
          sequence: 1,
          event: { type: "permission_request", text: "must not be dropped" },
        },
      },
      appendValidResult: true,
    },
    {
      name: "unknown Result status",
      notification: {
        jsonrpc: "2.0",
        method: "query/result",
        params: {
          queryId: "query-invalid",
          sessionId: "session-invalid",
          turnId: "turn-invalid",
          operationId: "operation-invalid",
          status: "partially_completed",
          output: { text: "invalid" },
        },
      },
      appendValidResult: false,
    },
  ] as const;

  for (const fixture of cases) {
    await context.test(fixture.name, async () => {
      const clientToHost = new PassThrough();
      const hostToClient = new PassThrough();
      const host = runInvalidNotificationFixtureHost(
        clientToHost,
        hostToClient,
        fixture.notification,
        fixture.appendValidResult,
      );
      const client = await createAgentClient(
        {
          readable: hostToClient,
          writable: clientToHost,
          close: async () => {
            clientToHost.end();
            await host;
          },
        },
        clientOptions,
      );

      try {
        const query = await client.query({ prompt: "reject protocol drift" });
        await assert.rejects(query.result(), (error: unknown) => {
          assert.ok(error instanceof SdkError);
          assert.equal(error.code, "process_lost");
          assert.equal(error.outcomeCertainty, "unknown");
          return true;
        });
      } finally {
        await client.close();
      }
    });
  }
});

async function runFixtureHost(
  requests: PassThrough,
  responses: PassThrough,
  initializeRequests: unknown[],
): Promise<void> {
  const lines = createInterface({ input: requests, crlfDelay: Infinity });
  for await (const line of lines) {
    const request = JSON.parse(line) as {
      id: number;
      method: string;
      params: Record<string, unknown>;
    };
    if (request.method === "initialize") {
      initializeRequests.push(request.params);
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: {
          protocolVersion: 6,
          runtimeVersion: "0.2.17",
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
    if (request.method === "query/start") {
      assert.deepEqual(request.params, {
        prompt: "hello\n\nfocus on the layout",
        images: ["screenshots/fixture.png"],
        outputSchema: {
          type: "object",
          properties: { summary: { type: "string" } },
          required: ["summary"],
        },
        sessionId: null,
        sessionName: null,
        agent: null,
        cwd: "D:/workspace/project",
        model: "sdk:openai:resolved",
      });
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: {
          queryId: "query-1",
          sessionId: "session-1",
          turnId: "turn-1",
          operationId: "operation-1",
          accepted: true,
          createdSession: true,
          sessionLifetime: "connection",
        },
      });
      write(responses, {
        jsonrpc: "2.0",
        method: "query/event",
        params: {
          queryId: "query-1",
          sessionId: "session-1",
          turnId: "turn-1",
          operationId: "operation-1",
          sequence: 1,
          event: {
            type: "tool_event",
            toolCallId: "tool-1",
            toolName: "Read",
            status: "started",
          },
        },
      });
      write(responses, {
        jsonrpc: "2.0",
        method: "query/event",
        params: {
          queryId: "query-1",
          sessionId: "session-1",
          turnId: "turn-1",
          operationId: "operation-1",
          sequence: 2,
          event: {
            type: "permission_request",
            requestId: "permission-1",
            action: "read",
            resources: ["README.md"],
            source: { kind: "tool_call", identity: "Read" },
            toolCallId: "tool-1",
            responseTimeoutMs: 120_000,
          },
        },
      });
      continue;
    }
    if (request.method === "permission/respond") {
      assert.deepEqual(request.params, {
        queryId: "query-1",
        sessionId: "session-1",
        turnId: "turn-1",
        operationId: "operation-1",
        requestId: "permission-1",
        decision: "allow_once",
      });
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { requestId: "permission-1", accepted: true },
      });
      write(responses, {
        jsonrpc: "2.0",
        method: "query/event",
        params: {
          queryId: "query-1",
          sessionId: "session-1",
          turnId: "turn-1",
          operationId: "operation-1",
          sequence: 3,
          event: {
            type: "tool_event",
            toolCallId: "tool-1",
            toolName: "Read",
            status: "completed",
            durationMs: 12,
          },
        },
      });
      write(responses, {
        jsonrpc: "2.0",
        method: "query/event",
        params: {
          queryId: "query-1",
          sessionId: "session-1",
          turnId: "turn-1",
          operationId: "operation-1",
          sequence: 4,
          event: { type: "assistant_text_delta", text: "fixture result" },
        },
      });
      write(responses, {
        jsonrpc: "2.0",
        method: "query/result",
        params: {
          queryId: "query-1",
          sessionId: "session-1",
          turnId: "turn-1",
          operationId: "operation-1",
          status: "completed",
          output: {
            text: "fixture result",
            structured: { summary: "fixture result" },
          },
          usage: {
            inputTokens: 100,
            outputTokens: 25,
            totalTokens: 125,
            cachedTokens: 40,
          },
        },
      });
      continue;
    }
    if (request.method === "session/close") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { sessionId: "session-1", unloaded: true },
      });
      continue;
    }
    if (request.method === "shutdown") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { accepted: true },
      });
      responses.end();
      return;
    }
    throw new Error(`Unexpected fixture method: ${request.method}`);
  }
}

async function runSessionFixtureHost(
  requests: PassThrough,
  responses: PassThrough,
  methods: string[],
): Promise<void> {
  const lines = createInterface({ input: requests, crlfDelay: Infinity });
  for await (const line of lines) {
    const request = JSON.parse(line) as {
      id: number;
      method: string;
      params: Record<string, unknown>;
    };
    methods.push(request.method);
    if (request.method === "initialize") {
      write(responses, initializeResponse(request.id));
      continue;
    }
    if (request.method === "session/create") {
      assert.deepEqual(request.params, {
        sessionName: null,
        agent: "agentic",
        cwd: "D:/workspace/project",
        model: "sdk:openai:resolved",
      });
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: {
          sessionId: "session-explicit",
          sessionName: "Explicit",
          agent: "agentic",
          lifetime: "durable",
        },
      });
      continue;
    }
    if (request.method === "query/start") {
      assert.deepEqual(request.params, {
        prompt: "",
        images: ["screenshots/continued.webp"],
        outputSchema: { type: "object" },
        sessionId: "session-explicit",
      });
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: {
          queryId: "query-explicit",
          sessionId: "session-explicit",
          turnId: "turn-explicit",
          operationId: "operation-explicit",
          accepted: true,
          createdSession: false,
          sessionLifetime: "connection",
        },
      });
      write(responses, {
        jsonrpc: "2.0",
        method: "query/result",
        params: {
          queryId: "query-explicit",
          sessionId: "session-explicit",
          turnId: "turn-explicit",
          operationId: "operation-explicit",
          status: "completed",
          output: { text: "continued" },
        },
      });
      continue;
    }
    if (request.method === "session/close") {
      assert.deepEqual(request.params, {
        sessionId: "session-explicit",
      });
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { sessionId: "session-explicit", unloaded: true },
      });
      continue;
    }
    if (request.method === "shutdown") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { accepted: true },
      });
      responses.end();
      return;
    }
    throw new Error(`Unexpected fixture method: ${request.method}`);
  }
}

async function runResumeFixtureHost(
  requests: PassThrough,
  responses: PassThrough,
  methods: string[],
): Promise<void> {
  const lines = createInterface({ input: requests, crlfDelay: Infinity });
  for await (const line of lines) {
    const request = JSON.parse(line) as {
      id: number;
      method: string;
      params: Record<string, unknown>;
    };
    methods.push(request.method);
    if (request.method === "initialize") {
      write(responses, initializeResponse(request.id));
      continue;
    }
    if (request.method === "session/resume") {
      assert.deepEqual(request.params, { sessionId: "session-persisted" });
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: {
          sessionId: "session-persisted",
          sessionName: "Persisted",
          agent: "agentic",
          lifetime: "durable",
          workspacePath: "D:/workspace/project",
        },
      });
      continue;
    }
    if (request.method === "session/close") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { sessionId: "session-persisted", unloaded: true },
      });
      continue;
    }
    if (request.method === "shutdown") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { accepted: true },
      });
      responses.end();
      return;
    }
    throw new Error(`Unexpected fixture method: ${request.method}`);
  }
}

async function runCancelFixtureHost(
  requests: PassThrough,
  responses: PassThrough,
  onCancel: () => void,
  emitEventAfterStart = false,
): Promise<void> {
  const lines = createInterface({ input: requests, crlfDelay: Infinity });
  for await (const line of lines) {
    const request = JSON.parse(line) as {
      id: number;
      method: string;
      params: Record<string, unknown>;
    };
    if (request.method === "initialize") {
      write(responses, initializeResponse(request.id));
      continue;
    }
    if (request.method === "query/start") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: {
          queryId: "query-cancel",
          sessionId: "session-cancel",
          turnId: "turn-cancel",
          operationId: "operation-cancel",
          accepted: true,
          createdSession: true,
          sessionLifetime: "connection",
        },
      });
      if (emitEventAfterStart) {
        write(responses, {
          jsonrpc: "2.0",
          method: "query/event",
          params: {
            queryId: "query-cancel",
            sessionId: "session-cancel",
            turnId: "turn-cancel",
            operationId: "operation-cancel",
            sequence: 1,
            event: { type: "assistant_text_delta", text: "partial" },
          },
        });
      }
      continue;
    }
    if (request.method === "query/cancel") {
      onCancel();
      assert.deepEqual(request.params, {
        queryId: "query-cancel",
        sessionId: "session-cancel",
        turnId: "turn-cancel",
        operationId: "operation-cancel",
      });
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: {
          queryId: "query-cancel",
          sessionId: "session-cancel",
          turnId: "turn-cancel",
          operationId: "operation-cancel",
          requested: true,
        },
      });
      write(responses, {
        jsonrpc: "2.0",
        method: "query/result",
        params: {
          queryId: "query-cancel",
          sessionId: "session-cancel",
          turnId: "turn-cancel",
          operationId: "operation-cancel",
          status: "cancelled",
          output: { text: "" },
          error: {
            message: "cancelled by caller",
            data: {
              code: "cancelled",
              stage: "query",
              retryable: false,
              correlationId: "operation-cancel",
              operationId: "operation-cancel",
              outcomeCertainty: "committed",
            },
          },
        },
      });
      continue;
    }
    if (request.method === "session/close") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { sessionId: "session-cancel", unloaded: true },
      });
      continue;
    }
    if (request.method === "shutdown") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { accepted: true },
      });
      responses.end();
      return;
    }
    throw new Error(`Unexpected fixture method: ${request.method}`);
  }
}

async function runProcessLossFixtureHost(
  requests: PassThrough,
  responses: PassThrough,
): Promise<void> {
  const lines = createInterface({ input: requests, crlfDelay: Infinity });
  for await (const line of lines) {
    const request = JSON.parse(line) as { id: number; method: string };
    if (request.method === "initialize") {
      write(responses, initializeResponse(request.id));
      continue;
    }
    if (request.method === "query/start") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: {
          queryId: "query-lost",
          sessionId: "session-lost",
          turnId: "turn-lost",
          operationId: "operation-lost",
          accepted: true,
          createdSession: true,
          sessionLifetime: "connection",
        },
      });
      responses.end();
      return;
    }
    throw new Error(`Unexpected fixture method: ${request.method}`);
  }
}

async function runClientCloseFixtureHost(
  requests: PassThrough,
  responses: PassThrough,
  methods: string[],
): Promise<void> {
  const lines = createInterface({ input: requests, crlfDelay: Infinity });
  for await (const line of lines) {
    const request = JSON.parse(line) as {
      id: number;
      method: string;
      params: Record<string, unknown>;
    };
    methods.push(request.method);
    if (request.method === "initialize") {
      write(responses, initializeResponse(request.id));
      continue;
    }
    if (request.method === "query/start") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: {
          queryId: "query-client-close",
          sessionId: "session-client-close",
          turnId: "turn-client-close",
          operationId: "operation-client-close",
          accepted: true,
          createdSession: true,
          sessionLifetime: "connection",
        },
      });
      continue;
    }
    if (request.method === "query/cancel") {
      assert.deepEqual(request.params, {
        queryId: "query-client-close",
        sessionId: "session-client-close",
        turnId: "turn-client-close",
        operationId: "operation-client-close",
      });
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: {
          queryId: "query-client-close",
          sessionId: "session-client-close",
          turnId: "turn-client-close",
          operationId: "operation-client-close",
          requested: true,
        },
      });
      write(responses, {
        jsonrpc: "2.0",
        method: "query/result",
        params: {
          queryId: "query-client-close",
          sessionId: "session-client-close",
          turnId: "turn-client-close",
          operationId: "operation-client-close",
          status: "cancelled",
          output: { text: "" },
          error: {
            message: "cancelled during client close",
            data: {
              code: "cancelled",
              stage: "query",
              retryable: false,
              correlationId: "operation-client-close",
              operationId: "operation-client-close",
              outcomeCertainty: "committed",
            },
          },
        },
      });
      continue;
    }
    if (request.method === "session/close") {
      assert.deepEqual(request.params, { sessionId: "session-client-close" });
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { sessionId: "session-client-close", unloaded: true },
      });
      continue;
    }
    if (request.method === "shutdown") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { accepted: true },
      });
      responses.end();
      return;
    }
    throw new Error(`Unexpected fixture method: ${request.method}`);
  }
}

async function runEmptyModelIdFixtureHost(
  requests: PassThrough,
  responses: PassThrough,
): Promise<void> {
  const lines = createInterface({ input: requests, crlfDelay: Infinity });
  for await (const line of lines) {
    const request = JSON.parse(line) as { id: number; method: string };
    assert.equal(request.method, "initialize");
    const response = initializeResponse(request.id) as {
      result: { modelId: string };
    };
    response.result.modelId = "";
    write(responses, response);
  }
  responses.end();
}

async function runOperationErrorFixtureHost(
  requests: PassThrough,
  responses: PassThrough,
): Promise<void> {
  const lines = createInterface({ input: requests, crlfDelay: Infinity });
  for await (const line of lines) {
    const request = JSON.parse(line) as { id: number; method: string };
    if (request.method === "initialize") {
      write(responses, initializeResponse(request.id));
      continue;
    }
    if (request.method === "query/start") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        error: {
          code: -32000,
          message: "authentication is required",
          data: {
            code: "action_required",
            stage: "query",
            retryable: false,
            correlationId: "request:query-start",
            outcomeCertainty: "not_started",
            recovery: "initialize",
          },
        },
      });
      continue;
    }
    if (request.method === "shutdown") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { accepted: true },
      });
      responses.end();
      return;
    }
    throw new Error(`Unexpected fixture method: ${request.method}`);
  }
}

async function runInvalidErrorFixtureHost(
  requests: PassThrough,
  responses: PassThrough,
): Promise<void> {
  const lines = createInterface({ input: requests, crlfDelay: Infinity });
  for await (const line of lines) {
    const request = JSON.parse(line) as { id: number; method: string };
    if (request.method === "initialize") {
      write(responses, initializeResponse(request.id));
      continue;
    }
    if (request.method === "query/start") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        error: {
          code: -32000,
          message: "unknown error contract",
          data: {
            code: "invented_error",
            stage: "query",
            retryable: false,
            correlationId: "request:invalid",
            outcomeCertainty: "not_started",
          },
        },
      });
      responses.end();
      return;
    }
    throw new Error(`Unexpected fixture method: ${request.method}`);
  }
}

async function runAmbiguousResponseFixtureHost(
  requests: PassThrough,
  responses: PassThrough,
): Promise<void> {
  const lines = createInterface({ input: requests, crlfDelay: Infinity });
  for await (const line of lines) {
    const request = JSON.parse(line) as { id: number; method: string };
    if (request.method === "initialize") {
      write(responses, initializeResponse(request.id));
      continue;
    }
    if (request.method === "query/start") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: {
          queryId: "query-ambiguous",
          sessionId: "session-ambiguous",
          turnId: "turn-ambiguous",
          operationId: "operation-ambiguous",
          accepted: true,
          createdSession: true,
          sessionLifetime: "connection",
        },
        error: {
          code: -32000,
          message: "must not coexist with result",
          data: {
            code: "internal",
            stage: "query",
            retryable: false,
            correlationId: "request:ambiguous",
            outcomeCertainty: "not_started",
          },
        },
      });
      responses.end();
      return;
    }
    throw new Error(`Unexpected fixture method: ${request.method}`);
  }
}

async function runInvalidNotificationFixtureHost(
  requests: PassThrough,
  responses: PassThrough,
  notification: unknown,
  appendValidResult: boolean,
): Promise<void> {
  const lines = createInterface({ input: requests, crlfDelay: Infinity });
  for await (const line of lines) {
    const request = JSON.parse(line) as { id: number; method: string };
    if (request.method === "initialize") {
      write(responses, initializeResponse(request.id));
      continue;
    }
    if (request.method === "query/start") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: {
          queryId: "query-invalid",
          sessionId: "session-invalid",
          turnId: "turn-invalid",
          operationId: "operation-invalid",
          accepted: true,
          createdSession: true,
          sessionLifetime: "connection",
        },
      });
      write(responses, notification);
      if (appendValidResult) {
        write(responses, {
          jsonrpc: "2.0",
          method: "query/result",
          params: {
            queryId: "query-invalid",
            sessionId: "session-invalid",
            turnId: "turn-invalid",
            operationId: "operation-invalid",
            status: "completed",
            output: { text: "must not become authoritative" },
          },
        });
      }
      continue;
    }
    if (request.method === "shutdown") {
      write(responses, {
        jsonrpc: "2.0",
        id: request.id,
        result: { accepted: true },
      });
      responses.end();
      return;
    }
  }
  responses.end();
}

function initializeResponse(id: number): unknown {
  return {
    jsonrpc: "2.0",
    id,
    result: {
      protocolVersion: 6,
      runtimeVersion: "0.2.17",
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
  };
}

function write(stream: PassThrough, value: unknown): void {
  stream.write(`${JSON.stringify(value)}\n`);
}
