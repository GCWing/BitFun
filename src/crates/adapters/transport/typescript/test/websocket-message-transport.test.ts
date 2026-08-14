import assert from "node:assert/strict";
import test from "node:test";

import {
  WebSocketMessageTransport,
  type WebSocketLike,
} from "../src/index.js";

test("WebSocket transport rejects before the browser buffer exceeds its cap", async () => {
  const socket = new TestWebSocket();
  socket.bufferedAmount = 10;
  const transport = createTransport(socket, { maxBufferedBytes: 10 });

  await assert.rejects(
    transport.send("x"),
    /send buffer capacity was exceeded/,
  );
  assert.deepEqual(socket.sent, []);
});

test("WebSocket transport bounds a send whose browser buffer never drains", async () => {
  const socket = new TestWebSocket();
  socket.bufferedAmount = 1;
  const transport = createTransport(socket, { drainTimeoutMs: 5 });

  await assert.rejects(
    transport.send("message"),
    /drain deadline was exceeded/,
  );
  assert.deepEqual(socket.sent, ["message"]);
});

test("WebSocket transport projects messages and carrier close exactly once", async () => {
  const socket = new TestWebSocket();
  const transport = createTransport(socket);
  const messages: string[] = [];
  const closes: unknown[] = [];
  transport.subscribe({
    message: (message) => messages.push(message),
    close: (cause) => closes.push(cause),
  });

  transport.receive("one");
  const cause = new Error("closed");
  socket.readyState = 3;
  transport.carrierClosed(cause);
  transport.carrierClosed(new Error("duplicate"));
  transport.receive("late");
  await Promise.all([transport.close(), transport.close()]);

  assert.deepEqual(messages, ["one"]);
  assert.deepEqual(closes, [cause]);
  assert.equal(socket.closeCalls, 0);
});

function createTransport(
  socket: TestWebSocket,
  overrides: Partial<{
    maxBufferedBytes: number;
    drainTimeoutMs: number;
  }> = {},
): WebSocketMessageTransport {
  return new WebSocketMessageTransport(socket, {
    maxBufferedBytes: overrides.maxBufferedBytes ?? 1_024,
    drainTimeoutMs: overrides.drainTimeoutMs ?? 100,
  });
}

class TestWebSocket implements WebSocketLike {
  readyState = 1;
  bufferedAmount = 0;
  readonly sent: string[] = [];
  closeCalls = 0;

  send(message: string): void {
    this.sent.push(message);
  }

  close(): void {
    this.closeCalls += 1;
    this.readyState = 3;
  }
}
