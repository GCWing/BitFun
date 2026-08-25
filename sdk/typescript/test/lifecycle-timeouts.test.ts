import assert from "node:assert/strict";
import { createInterface } from "node:readline";
import { PassThrough } from "node:stream";
import test from "node:test";

import { SdkError } from "../src/errors.js";
import { createAgentClient } from "../src/internal/client.js";
import { JsonRpcConnection } from "../src/internal/json-rpc.js";
import type {
  QueryStartResult,
  SessionCreateResult,
} from "../src/internal/wire/index.js";
import { Query } from "../src/query.js";
import { Session } from "../src/session.js";

const model = {
  provider: "openai" as const,
  model: "fixture-model",
  apiKey: "fixture-secret",
  baseUrl: "http://127.0.0.1:43123/v1",
};

test("client initialization aborts the Host connection after its startup deadline", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  let transportClosed = false;
  const starting = createAgentClient(
    {
      readable: hostToClient,
      writable: clientToHost,
      close: async () => {
        transportClosed = true;
        clientToHost.end();
        hostToClient.end();
      },
    },
    { cwd: "D:/workspace/project", initializeTimeoutMs: 20, model },
  );

  await assert.rejects(withTestDeadline(starting), (error: unknown) => {
    assert.ok(error instanceof SdkError);
    assert.equal(error.code, "timeout");
    assert.equal(error.stage, "initialize");
    assert.equal(error.outcomeCertainty, "unknown");
    assert.equal(error.recovery, "restart_host");
    return true;
  });
  assert.equal(transportClosed, true);
});

test("Query.close aborts an unresponsive Host after its cleanup deadline", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  let transportClosed = false;
  const connection = new JsonRpcConnection({
    readable: hostToClient,
    writable: clientToHost,
    close: async () => {
      transportClosed = true;
      clientToHost.end();
      hostToClient.end();
    },
  });
  const started: QueryStartResult = {
    queryId: "query-timeout",
    sessionId: "session-timeout",
    turnId: "turn-timeout",
    operationId: "operation-timeout",
    accepted: true,
    createdSession: true,
    sessionLifetime: "connection",
  };
  const createQuery = Query.create as unknown as (
    owner: JsonRpcConnection,
    accepted: QueryStartResult,
    closeTimeoutMs: number,
  ) => Query;
  const query = createQuery(connection, started, 20);
  void query.result().catch(() => {});
  const closing = query.close();

  try {
    await assert.rejects(withTestDeadline(closing), isCleanupTimeout);
    assert.equal(transportClosed, true);
  } finally {
    hostToClient.end();
    await closing.catch(() => {});
  }
});

test("connection shutdown closes transport when the Host never responds", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  let transportClosed = false;
  const connection = new JsonRpcConnection({
    readable: hostToClient,
    writable: clientToHost,
    close: async () => {
      transportClosed = true;
      clientToHost.end();
      hostToClient.end();
    },
  });
  const shutdown = connection.shutdown as unknown as (timeoutMs: number) => Promise<void>;
  const closing = shutdown.call(connection, 20);

  try {
    await assert.rejects(withTestDeadline(closing), isCleanupTimeout);
    assert.equal(transportClosed, true);
  } finally {
    hostToClient.end();
    await closing.catch(() => {});
  }
});

test("Session.close aborts an unresponsive Host after its cleanup deadline", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  let transportClosed = false;
  const connection = new JsonRpcConnection({
    readable: hostToClient,
    writable: clientToHost,
    close: async () => {
      transportClosed = true;
      clientToHost.end();
      hostToClient.end();
    },
  });
  const created: SessionCreateResult = {
    sessionId: "session-timeout",
    sessionName: "timeout",
    agent: "agentic",
    lifetime: "durable",
  };
  const createSession = Session.create as unknown as (
    owner: JsonRpcConnection,
    accepted: SessionCreateResult,
    onQuery: (query: Query) => Query,
    closeTimeoutMs: number,
  ) => Session;
  const session = createSession(connection, created, (query) => query, 20);
  const closing = session.close();

  try {
    await assert.rejects(withTestDeadline(closing), isCleanupTimeout);
    assert.equal(transportClosed, true);
  } finally {
    hostToClient.end();
    await closing.catch(() => {});
  }
});

test("implicit Session cleanup with unknown outcome makes the Query connection unusable", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  let transportClosed = false;
  const connection = new JsonRpcConnection({
    readable: hostToClient,
    writable: clientToHost,
    close: async () => {
      transportClosed = true;
      clientToHost.end();
      hostToClient.end();
    },
  });
  const started: QueryStartResult = {
    queryId: "query-uncertain-session-cleanup",
    sessionId: "session-uncertain-cleanup",
    turnId: "turn-uncertain-cleanup",
    operationId: "operation-uncertain-cleanup",
    accepted: true,
    createdSession: true,
    sessionLifetime: "connection",
  };
  const query = Query.create(connection, started);
  write(hostToClient, {
    jsonrpc: "2.0",
    method: "query/result",
    params: {
      queryId: started.queryId,
      sessionId: started.sessionId,
      turnId: started.turnId,
      operationId: started.operationId,
      status: "completed",
      output: { text: "done" },
    },
  });
  await query.result();
  const host = respondWithUncertainSessionCleanup(clientToHost, hostToClient);

  try {
    await assert.rejects(query.close(), (error: unknown) => {
      assert.ok(error instanceof SdkError);
      assert.equal(error.code, "cleanup_required");
      assert.equal(error.outcomeCertainty, "unknown");
      assert.equal(error.recovery, "restart_host");
      return true;
    });
    assert.equal(transportClosed, true);
  } finally {
    hostToClient.end();
    await host;
  }
});

function isCleanupTimeout(error: unknown): boolean {
  assert.ok(error instanceof SdkError);
  assert.equal(error.code, "cleanup_required");
  assert.equal(error.outcomeCertainty, "unknown");
  assert.equal(error.recovery, "restart_host");
  return true;
}

async function withTestDeadline<T>(promise: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const expired = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => reject(new Error("cleanup did not honor its deadline")), 250);
  });
  try {
    return await Promise.race([promise, expired]);
  } finally {
    if (timer !== undefined) {
      clearTimeout(timer);
    }
  }
}

async function respondWithUncertainSessionCleanup(
  requests: PassThrough,
  responses: PassThrough,
): Promise<void> {
  const lines = createInterface({ input: requests, crlfDelay: Infinity });
  for await (const line of lines) {
    const request = JSON.parse(line) as { id: number; method: string };
    assert.equal(request.method, "session/close");
    write(responses, {
      jsonrpc: "2.0",
      id: request.id,
      error: {
        code: -32000,
        message: "cleanup outcome is unknown",
        data: {
          code: "cleanup_required",
          stage: "session",
          retryable: false,
          correlationId: "session:uncertain-cleanup",
          outcomeCertainty: "unknown",
          recovery: "restart_host",
        },
      },
    });
    return;
  }
}

function write(stream: PassThrough, value: unknown): void {
  stream.write(`${JSON.stringify(value)}\n`);
}
