import assert from "node:assert/strict";
import test from "node:test";

import {
  JsonRpcLimitError,
  JsonRpcPeer,
  JsonRpcProtocolError,
  JsonRpcRemoteError,
  JsonRpcTimeoutError,
  type MessageTransport,
  type MessageTransportObserver,
} from "../src/index.js";

test("correlates one strict JSON-RPC response with its request", async () => {
  const transport = new TestMessageTransport();
  const peer = createPeer(transport);

  const response = peer.request("example/read", { value: 7 });
  assert.deepEqual(JSON.parse(transport.sent[0] ?? ""), {
    jsonrpc: "2.0",
    id: 1,
    method: "example/read",
    params: { value: 7 },
  });

  transport.receive({ jsonrpc: "2.0", id: 1, result: { accepted: true } });
  assert.deepEqual(await response, { accepted: true });
});

test("rejects an ambiguous response envelope and fails every pending request", async () => {
  const transport = new TestMessageTransport();
  const peer = createPeer(transport);
  const first = peer.request("example/first", {});
  const second = peer.request("example/second", {});

  transport.receive({
    jsonrpc: "2.0",
    id: 1,
    result: null,
    error: { code: -32000, message: "ambiguous" },
  });

  await assert.rejects(first, JsonRpcProtocolError);
  await assert.rejects(second, JsonRpcProtocolError);
  assert.equal(transport.closeCalls, 1);
});

test("projects a strict remote error without inventing application policy", async () => {
  const transport = new TestMessageTransport();
  const peer = createPeer(transport);
  const response = peer.request("example/fail", {});

  transport.receive({
    jsonrpc: "2.0",
    id: 1,
    error: { code: -32001, message: "denied", data: { reason: "policy" } },
  });

  await assert.rejects(response, (error: unknown) => {
    assert.ok(error instanceof JsonRpcRemoteError);
    assert.equal(error.code, -32001);
    assert.deepEqual(error.data, { reason: "policy" });
    return true;
  });
});

test("a request deadline clears only that request and leaves the carrier reusable", async () => {
  const transport = new TestMessageTransport();
  const peer = createPeer(transport, { requestTimeoutMs: 15 });
  const first = peer.request("example/slow", {});

  await assert.rejects(first, JsonRpcTimeoutError);
  assert.equal(transport.closeCalls, 0);

  const second = peer.request("example/next", {});
  transport.receive({ jsonrpc: "2.0", id: 2, result: "ready" });
  assert.equal(await second, "ready");
});

test("ignores one late response for a timed-out request id", async () => {
  const transport = new TestMessageTransport();
  const peer = createPeer(transport, { requestTimeoutMs: 15 });
  const expired = peer.request("example/slow", {});
  await assert.rejects(expired, JsonRpcTimeoutError);

  transport.receive({ jsonrpc: "2.0", id: 1, result: "late" });
  const next = peer.request("example/next", {});
  transport.receive({ jsonrpc: "2.0", id: 2, result: "current" });

  assert.equal(await next, "current");
  assert.equal(transport.closeCalls, 0);
});

test("rejects an id generator that reuses a tombstoned request id", async () => {
  const transport = new TestMessageTransport();
  const peer = new JsonRpcPeer(transport, {
    createRequestId: () => 1,
    requestTimeoutMs: 15,
    limits: {
      maxMessageBytes: 1_024,
      maxPendingRequests: 8,
      maxPendingBytes: 4_096,
      maxOutboundBytes: 4_096,
    },
  });
  await assert.rejects(peer.request("example/slow", {}), JsonRpcTimeoutError);

  await assert.rejects(
    peer.request("example/reused", {}),
    /request id must be unique/,
  );
});

test("fails closed instead of evicting an unresolved timeout tombstone", async () => {
  const transport = new TestMessageTransport();
  let nextId = 1;
  const peer = new JsonRpcPeer(transport, {
    createRequestId: () => nextId++,
    requestTimeoutMs: 15,
    limits: {
      maxMessageBytes: 1_024,
      maxPendingRequests: 1,
      maxPendingBytes: 4_096,
      maxOutboundBytes: 4_096,
    },
  });
  await assert.rejects(peer.request("example/first", {}), JsonRpcTimeoutError);

  await assert.rejects(peer.request("example/second", {}), JsonRpcProtocolError);
  assert.equal(transport.closeCalls, 1);
});

test("rejects a request before send when the pending count is exhausted", async () => {
  const transport = new TestMessageTransport();
  const peer = createPeer(transport, { maxPendingRequests: 1 });
  const first = peer.request("example/first", {});

  await assert.rejects(
    peer.request("example/second", {}),
    (error: unknown) => isLimit(error, "pending_count"),
  );
  assert.equal(transport.sent.length, 1);

  transport.receive({ jsonrpc: "2.0", id: 1, result: null });
  await first;
});

test("rejects a request before send when retained pending bytes are exhausted", async () => {
  const transport = new TestMessageTransport();
  const peer = createPeer(transport, { maxPendingBytes: 100 });
  const first = peer.request("example/first", { value: "x".repeat(20) });

  await assert.rejects(
    peer.request("example/second", { value: "x".repeat(20) }),
    (error: unknown) => isLimit(error, "pending_bytes"),
  );
  assert.equal(transport.sent.length, 1);

  transport.receive({ jsonrpc: "2.0", id: 1, result: null });
  await first;
});

test("bounds bytes waiting on a backpressured carrier", async () => {
  const transport = new TestMessageTransport();
  transport.blockSends();
  const peer = createPeer(transport, {
    maxOutboundBytes: 100,
    maxPendingBytes: 1_000,
  });
  const first = peer.request("example/first", { value: "x".repeat(20) });

  await assert.rejects(
    peer.request("example/second", { value: "x".repeat(20) }),
    (error: unknown) => isLimit(error, "outbound_bytes"),
  );
  assert.equal(transport.sent.length, 1);

  transport.releaseSends();
  transport.receive({ jsonrpc: "2.0", id: 1, result: null });
  await first;
});

test("fails closed when an inbound carrier message exceeds its byte budget", async () => {
  const transport = new TestMessageTransport();
  const peer = createPeer(transport, { maxMessageBytes: 64 });
  const response = peer.request("example/read", {});

  transport.receiveRaw(JSON.stringify({
    jsonrpc: "2.0",
    id: 1,
    result: "x".repeat(80),
  }));

  await assert.rejects(response, JsonRpcProtocolError);
  assert.equal(transport.closeCalls, 1);
});

test("delivers strict notifications without owning their application fanout", async () => {
  const transport = new TestMessageTransport();
  const peer = createPeer(transport);
  const notifications: Array<{ method: string; params: unknown }> = [];
  peer.onNotification((notification) => notifications.push(notification));

  transport.receive({
    jsonrpc: "2.0",
    method: "example/event",
    params: { sequence: 1 },
  });

  assert.deepEqual(notifications, [
    { method: "example/event", params: { sequence: 1 } },
  ]);
});

test("isolates an application notification listener failure from the wire peer", async () => {
  const transport = new TestMessageTransport();
  const notificationErrors: Error[] = [];
  const peer = createPeer(transport, { notificationErrors });
  peer.onNotification(() => {
    throw new Error("consumer failed");
  });

  transport.receive({
    jsonrpc: "2.0",
    method: "example/event",
    params: { sequence: 1 },
  });
  const response = peer.request("example/next", {});
  transport.receive({ jsonrpc: "2.0", id: 1, result: "ready" });

  assert.equal(await response, "ready");
  assert.equal(notificationErrors[0]?.message, "consumer failed");
  assert.equal(transport.closeCalls, 0);
});

test("a malformed remote error rejects the correlated request as a protocol failure", async () => {
  const transport = new TestMessageTransport();
  const peer = createPeer(transport);
  const response = peer.request("example/fail", {});

  transport.receive({
    jsonrpc: "2.0",
    id: 1,
    error: { code: "not-an-integer", message: "invalid" },
  });

  await assert.rejects(response, JsonRpcProtocolError);
  assert.equal(transport.closeCalls, 1);
});

test("graceful close rejects pending requests and closes the carrier exactly once", async () => {
  const transport = new TestMessageTransport();
  const peer = createPeer(transport);
  const pending = peer.request("example/pending", {});

  await Promise.all([peer.close(), peer.close()]);

  await assert.rejects(pending, /JSON-RPC peer closed/);
  assert.equal(transport.closeCalls, 1);
});

test("a failure listener cannot prevent pending cleanup or carrier close", async () => {
  const transport = new TestMessageTransport();
  const peer = createPeer(transport);
  peer.onFailure(() => {
    throw new Error("diagnostic listener failed");
  });
  const pending = peer.request("example/pending", {});

  assert.doesNotThrow(() => transport.end(new Error("carrier lost")));

  await assert.rejects(pending, /carrier closed/);
  await peer.close();
  assert.equal(transport.closeCalls, 1);
});

test("a carrier that closes during subscription is reclaimed without a constructor race", async () => {
  const transport = new SynchronouslyClosingTransport();
  const peer = new JsonRpcPeer(transport, {
    createRequestId: () => 1,
    requestTimeoutMs: 1_000,
    limits: {
      maxMessageBytes: 1_024,
      maxPendingRequests: 8,
      maxPendingBytes: 4_096,
      maxOutboundBytes: 4_096,
    },
  });

  await assert.rejects(peer.request("example/read", {}), /carrier closed/);
  await peer.close();
  assert.equal(transport.unsubscribeCalls, 1);
  assert.equal(transport.closeCalls, 1);
});

interface PeerOverrides {
  requestTimeoutMs?: number;
  maxMessageBytes?: number;
  maxPendingRequests?: number;
  maxPendingBytes?: number;
  maxOutboundBytes?: number;
  notificationErrors?: Error[];
}

function createPeer(
  transport: TestMessageTransport,
  overrides: PeerOverrides = {},
): JsonRpcPeer {
  let nextId = 1;
  return new JsonRpcPeer(transport, {
    createRequestId: () => nextId++,
    requestTimeoutMs: overrides.requestTimeoutMs ?? 1_000,
    limits: {
      maxMessageBytes: overrides.maxMessageBytes ?? 1_024,
      maxPendingRequests: overrides.maxPendingRequests ?? 8,
      maxPendingBytes: overrides.maxPendingBytes ?? 4_096,
      maxOutboundBytes: overrides.maxOutboundBytes ?? 4_096,
    },
    onNotificationError: (error) => overrides.notificationErrors?.push(error),
  });
}

function isLimit(error: unknown, limit: JsonRpcLimitError["limit"]): boolean {
  assert.ok(error instanceof JsonRpcLimitError);
  assert.equal(error.limit, limit);
  return true;
}

class TestMessageTransport implements MessageTransport {
  readonly sent: string[] = [];
  closeCalls = 0;
  #observer?: MessageTransportObserver;
  #sendGate?: Promise<void>;
  #releaseSend?: () => void;

  subscribe(observer: MessageTransportObserver): () => void {
    assert.equal(this.#observer, undefined);
    this.#observer = observer;
    return () => {
      if (this.#observer === observer) {
        this.#observer = undefined;
      }
    };
  }

  async send(message: string): Promise<void> {
    this.sent.push(message);
    await this.#sendGate;
  }

  async close(): Promise<void> {
    this.closeCalls += 1;
  }

  receive(value: unknown): void {
    this.receiveRaw(JSON.stringify(value));
  }

  receiveRaw(message: string): void {
    this.#observer?.message(message);
  }

  end(cause?: unknown): void {
    this.#observer?.close(cause);
  }

  blockSends(): void {
    this.#sendGate = new Promise((resolve) => {
      this.#releaseSend = resolve;
    });
  }

  releaseSends(): void {
    this.#releaseSend?.();
    this.#releaseSend = undefined;
    this.#sendGate = undefined;
  }
}

class SynchronouslyClosingTransport implements MessageTransport {
  unsubscribeCalls = 0;
  closeCalls = 0;

  subscribe(observer: MessageTransportObserver): () => void {
    observer.close(new Error("already closed"));
    return () => {
      this.unsubscribeCalls += 1;
    };
  }

  async send(): Promise<void> {}

  async close(): Promise<void> {
    this.closeCalls += 1;
  }
}
