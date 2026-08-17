import assert from "node:assert/strict";
import { PassThrough, Writable } from "node:stream";
import test from "node:test";

import { SdkError } from "../src/errors.js";
import { JsonRpcConnection } from "../src/internal/json-rpc.js";
import type { QueryStartResult } from "../src/internal/wire/index.js";
import { Query } from "../src/query.js";

test("request correlation rejects overload before the pending request count grows", async () => {
  const clientToHost = new PassThrough();
  const hostToClient = new PassThrough();
  const connection = createBoundedConnection(
    hostToClient,
    clientToHost,
    { maxPendingRequests: 1 },
  );
  const first = connection.request("query/start", {});

  try {
    await assert.rejects(connection.request("query/start", {}), isLocalOverload);
  } finally {
    void first.catch(() => {});
    await connection.abort(testCleanupError());
  }
});

test("request correlation rejects overload while the carrier remains backpressured", async () => {
  const hostToClient = new PassThrough();
  const stalled = new Writable({
    write(_chunk, _encoding, _callback) {
      // Deliberately retain the callback: the carrier has not accepted the
      // complete message, so MessageTransport.send must remain backpressured.
    },
  });
  const connection = createBoundedConnection(
    hostToClient,
    stalled,
    { maxOutboundBytes: 100 },
  );
  const first = connection.request("query/start", {});

  try {
    await assert.rejects(connection.request("query/start", {}), isLocalOverload);
  } finally {
    void first.catch(() => {});
    await connection.abort(testCleanupError());
  }
});

test("a slow Query consumer fails closed before its event queue grows without bound", async () => {
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
    queryId: "query-slow-consumer",
    sessionId: "session-slow-consumer",
    turnId: "turn-slow-consumer",
    operationId: "operation-slow-consumer",
    accepted: true,
    createdSession: false,
    sessionLifetime: "connection",
  };
  const query = Query.create(connection, started);

  for (let sequence = 1; sequence <= 1_025; sequence += 1) {
    write(hostToClient, {
      jsonrpc: "2.0",
      method: "query/event",
      params: {
        queryId: started.queryId,
        sessionId: started.sessionId,
        turnId: started.turnId,
        operationId: started.operationId,
        sequence,
        event: { type: "assistant_text_delta", text: "x" },
      },
    });
  }
  write(hostToClient, {
    jsonrpc: "2.0",
    method: "query/result",
    params: {
      queryId: started.queryId,
      sessionId: started.sessionId,
      turnId: started.turnId,
      operationId: started.operationId,
      status: "completed",
      output: { text: "x".repeat(1_025) },
    },
  });

  try {
    await assert.rejects(query.result(), (error: unknown) => {
      assert.ok(error instanceof SdkError);
      assert.equal(error.code, "overloaded");
      assert.equal(error.stage, "query");
      assert.equal(error.outcomeCertainty, "unknown");
      return true;
    });
    assert.equal(transportClosed, true);
  } finally {
    await connection.abort(
      new SdkError("test cleanup", {
        code: "process_lost",
        stage: "protocol",
        retryable: false,
        correlationId: "test:cleanup",
        outcomeCertainty: "unknown",
      }),
    );
  }
});

test("a protocol-sized Result remains available without consuming the event stream", async () => {
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
    queryId: "query-result-only",
    sessionId: "session-result-only",
    turnId: "turn-result-only",
    operationId: "operation-result-only",
    accepted: true,
    createdSession: false,
    sessionLifetime: "connection",
  };
  const query = Query.create(connection, started);
  const output = "x".repeat(512 * 1024);

  write(hostToClient, {
    jsonrpc: "2.0",
    method: "query/event",
    params: {
      queryId: started.queryId,
      sessionId: started.sessionId,
      turnId: started.turnId,
      operationId: started.operationId,
      sequence: 1,
      event: { type: "assistant_text_delta", text: output },
    },
  });
  write(hostToClient, {
    jsonrpc: "2.0",
    method: "query/result",
    params: {
      queryId: started.queryId,
      sessionId: started.sessionId,
      turnId: started.turnId,
      operationId: started.operationId,
      status: "completed",
      output: { text: output },
    },
  });

  try {
    const result = await withTestDeadline(query.result());
    assert.equal(result.status, "completed");
    assert.equal(result.outputText.length, output.length);
    assert.equal(transportClosed, false);
  } finally {
    await connection.abort(
      new SdkError("test cleanup", {
        code: "process_lost",
        stage: "protocol",
        retryable: false,
        correlationId: "test:cleanup",
        outcomeCertainty: "unknown",
      }),
    );
  }
});

test("an oversized unterminated Host frame fails the connection before unbounded buffering", async () => {
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
  const pending = connection.request("initialize", {});
  hostToClient.write("x".repeat(1024 * 1024 + 1));

  try {
    await assert.rejects(withTestDeadline(pending), (error: unknown) => {
      assert.ok(error instanceof SdkError);
      assert.equal(error.code, "process_lost");
      assert.equal(error.outcomeCertainty, "unknown");
      return true;
    });
    assert.equal(transportClosed, true);
  } finally {
    await connection.abort(
      new SdkError("test cleanup", {
        code: "process_lost",
        stage: "protocol",
        retryable: false,
        correlationId: "test:cleanup",
        outcomeCertainty: "unknown",
      }),
    );
  }
});

function write(stream: PassThrough, value: unknown): void {
  stream.write(`${JSON.stringify(value)}\n`);
}

async function withTestDeadline<T>(promise: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const expired = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => reject(new Error("frame buffering did not stop")), 250);
  });
  try {
    return await Promise.race([promise, expired]);
  } finally {
    if (timer !== undefined) {
      clearTimeout(timer);
    }
  }
}

interface TestConnectionLimits {
  maxPendingRequests?: number;
  maxOutboundBytes?: number;
}

function createBoundedConnection(
  readable: PassThrough,
  writable: Writable,
  limits: TestConnectionLimits,
): JsonRpcConnection {
  const Connection = JsonRpcConnection as unknown as new (
    transport: {
      readable: PassThrough;
      writable: Writable;
      close(): Promise<void>;
    },
    options: {
      requestTimeoutMs: number;
      limits: {
        maxMessageBytes: number;
        maxPendingRequests: number;
        maxPendingBytes: number;
        maxOutboundBytes: number;
      };
    },
  ) => JsonRpcConnection;
  return new Connection(
    {
      readable,
      writable,
      close: async () => {
        readable.end();
        writable.destroy();
      },
    },
    {
      requestTimeoutMs: 1_000,
      limits: {
        maxMessageBytes: 1_024,
        maxPendingRequests: limits.maxPendingRequests ?? 8,
        maxPendingBytes: 4_096,
        maxOutboundBytes: limits.maxOutboundBytes ?? 4_096,
      },
    },
  );
}

function isLocalOverload(error: unknown): boolean {
  assert.ok(error instanceof SdkError);
  assert.equal(error.code, "overloaded");
  assert.equal(error.stage, "query");
  assert.equal(error.outcomeCertainty, "not_started");
  return true;
}

function testCleanupError(): SdkError {
  return new SdkError("test cleanup", {
    code: "process_lost",
    stage: "protocol",
    retryable: false,
    correlationId: "test:cleanup",
    outcomeCertainty: "unknown",
  });
}
