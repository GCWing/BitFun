import { describe, expect, test } from "bun:test"
import { RpcError, RpcPeer, RpcProtocolError, connectRpcPeer, encodeFrame } from "../src/rpc"
import { HeaderSchema, MAX_MAX_FRAME_BYTES, MAX_STREAM_CHUNK_BYTES } from "../src/protocol"
import { StreamRegistry, remoteReadable } from "../src/streams"
import { registerStreamMethods } from "../src/stream-methods"

describe("protocol schema", () => {
  test("keeps header pairs exactly two strings at runtime and in generated JSON Schema", async () => {
    expect(HeaderSchema.safeParse(["name", "value"]).success).toBe(true)
    expect(HeaderSchema.safeParse(["name"]).success).toBe(false)
    expect(HeaderSchema.safeParse(["name", "value", "extra"]).success).toBe(false)

    const schema = await Bun.file(new URL("../protocol.schema.json", import.meta.url)).json()
    expect(schema.$defs.BackendHttpRequestParams.properties.headers.items).toMatchObject({
      minItems: 2,
      maxItems: 2,
      type: "array",
      items: { type: "string" },
    })
  })
})

describe("RpcPeer", () => {
  test("frames JSON with a four-byte big-endian length", () => {
    const frame = encodeFrame({ jsonrpc: "2.0", method: "ping", params: {} })
    expect(new DataView(frame.buffer, frame.byteOffset, 4).getUint32(0, false)).toBe(frame.byteLength - 4)
    expect(JSON.parse(new TextDecoder().decode(frame.subarray(4)))).toEqual({
      jsonrpc: "2.0",
      method: "ping",
      params: {},
    })
  })

  test("supports concurrent, out-of-order, and reentrant requests", async () => {
    const { host, backend } = peerPair()
    host.handle("host.decorate", ({ value }: { value: string }) => ({ value: `${value}:host` }))
    backend.handle("backend.work", async ({ value, wait }: { value: string; wait: boolean }) => {
      if (wait) await new Promise((resolve) => setTimeout(resolve, 10))
      return hostResult(await backend.request<{ value: string }>("host.decorate", { value }))
    })

    const slow = host.request<{ value: string }>("backend.work", { value: "slow", wait: true })
    const fast = host.request<{ value: string }>("backend.work", { value: "fast", wait: false })
    expect(await fast).toEqual({ value: "fast:host:backend" })
    expect(await slow).toEqual({ value: "slow:host:backend" })
    host.close()
    backend.close()
  })

  test("parses frames split at arbitrary byte boundaries", async () => {
    let right: RpcPeer
    const left = new RpcPeer({ write: (data) => deliver(right, data, 1) }, { idPrefix: "host" })
    right = new RpcPeer({ write: (data) => deliver(left, data, 2) }, { idPrefix: "backend" })
    right.handle("backend.echo", (params) => params)
    expect(
      await left.request<{ unicode: string; list: number[] }>("backend.echo", { unicode: "你好", list: [1, 2, 3] }),
    ).toEqual({
      unicode: "你好",
      list: [1, 2, 3],
    })
  })

  test("preserves numeric error code and JSON-compatible data", async () => {
    const { host, backend } = peerPair()
    backend.handle("backend.fail", () => {
      throw Object.assign(new Error("not ready"), { code: -32042, data: { kind: "not_ready", retry: true } })
    })
    const error = (await host.request("backend.fail").catch((value) => value)) as RpcError
    expect(error).toBeInstanceOf(RpcError)
    expect(error).toMatchObject({ code: -32042, message: "not ready", data: { kind: "not_ready", retry: true } })
  })

  test("serializes unexpected errors as internal errors with diagnostics", async () => {
    const { host, backend } = peerPair()
    backend.handle("backend.fail", () => {
      throw new TypeError("broken plugin")
    })
    const error = (await host.request("backend.fail").catch((value) => value)) as RpcError
    expect(error).toBeInstanceOf(RpcError)
    expect(error.code).toBe(-32603)
    expect(error.data).toMatchObject({ name: "TypeError", message: "broken plugin" })
    expect((error.data as { stack: string }).stack).toContain("broken plugin")
  })

  test("returns method-not-found without closing the connection", async () => {
    const { host, backend } = peerPair()
    const error = await host.request("backend.missing").catch((value) => value)
    expect(error).toMatchObject({ code: -32601 })
    backend.handle("backend.ok", () => "ok")
    expect(await host.request<string>("backend.ok")).toBe("ok")
  })

  test("returns a compact error for an oversized response and keeps the connection open", async () => {
    const { host, backend } = peerPair({ maxFrameBytes: 256 })
    backend.handle("backend.large", () => ({ value: "x".repeat(1_024) }))
    backend.handle("backend.ok", () => "ok")

    const error = await host.request("backend.large").catch((value) => value)

    expect(error).toMatchObject({ code: -32000, data: { kind: "response_too_large", maxFrameBytes: 256 } })
    expect(await host.request<string>("backend.ok")).toBe("ok")
  })

  test("rejects oversized frames before waiting for their payload", async () => {
    const errors: Error[] = []
    const peer = new RpcPeer(
      { write() {}, terminate() {} },
      { idPrefix: "host", maxFrameBytes: 32, onError: (error) => errors.push(error) },
    )
    const header = new Uint8Array(4)
    new DataView(header.buffer).setUint32(0, 33, false)
    peer.receive(header)
    await peer.closed
    expect(peer.closeError).toBeInstanceOf(RpcProtocolError)
    expect(errors).toHaveLength(1)
  })

  test("rejects malformed JSON and calls the EOF callback once", async () => {
    let eof = 0
    const peer = new RpcPeer(
      { write() {}, terminate() {} },
      { idPrefix: "host", onEof: () => void eof++, onError() {} },
    )
    const payload = new TextEncoder().encode("{")
    const frame = new Uint8Array(5)
    new DataView(frame.buffer).setUint32(0, 1, false)
    frame.set(payload, 4)
    peer.receive(frame)
    peer.end()
    await peer.closed
    await Promise.resolve()
    expect(peer.closeError).toMatchObject({ code: -32700 })
    expect(eof).toBe(1)
  })

  test("rejects pending requests when the transport reaches EOF", async () => {
    const peer = new RpcPeer({ write() {} }, { idPrefix: "host" })
    const request = peer.request("backend.never")
    peer.end()
    expect(((await request.catch((error) => error)) as Error).name).toBe("RpcConnectionClosedError")
  })

  test("validates negotiated and outbound frame limits", () => {
    const peer = new RpcPeer({ write() {} }, { idPrefix: "host" })
    expect(() => peer.setMaxFrameBytes(MAX_MAX_FRAME_BYTES + 1)).toThrow(RangeError)
    expect(() => encodeFrame({ value: "too large" }, 4)).toThrow(RangeError)
    expect(() => encodeFrame({ value: Number.NaN })).toThrow(TypeError)
  })

  test("connects over a real Bun TCP socket", async () => {
    const accepted = Promise.withResolvers<RpcPeer>()
    const peers = new WeakMap<object, RpcPeer>()
    const server = Bun.listen({
      hostname: "127.0.0.1",
      port: 0,
      socket: {
        open(socket) {
          const peer = new RpcPeer(socket, { idPrefix: "backend" })
          peers.set(socket, peer)
          peer.handle("backend.ping", ({ value }: { value: number }) => ({ value }))
          accepted.resolve(peer)
        },
        data(socket, data) {
          peers.get(socket)?.receive(data)
        },
        close(socket) {
          peers.get(socket)?.end()
        },
        error(socket, error) {
          peers.get(socket)?.end(error)
        },
      },
    })
    const client = await connectRpcPeer(`127.0.0.1:${server.port}`)
    const backend = await accepted.promise
    expect(await client.request<{ value: number }>("backend.ping", { value: 42 })).toEqual({ value: 42 })
    client.close()
    backend.close()
    server.stop(true)
  })
})

describe("stream method ownership", () => {
  test("removes the owner mapping when a stream read fails", async () => {
    const handlers = new Map<string, (value: unknown) => unknown | Promise<unknown>>()
    const registry = new StreamRegistry("host")
    const owners = new Map<string, string>()
    registerStreamMethods({ handle: (method, handler) => handlers.set(method, handler) }, registry, owners)
    const descriptor = registry.register(
      new ReadableStream<Uint8Array>({
        pull(controller) {
          controller.error(new Error("stream failed"))
        },
      }),
    )
    owners.set(descriptor.streamID, "instance")

    await expect(handlers.get("host.stream.read")!({
      instanceID: "instance",
      streamID: descriptor.streamID,
      maxBytes: 1,
    })).rejects.toThrow("stream failed")
    expect(owners.has(descriptor.streamID)).toBe(false)
  })
})

describe("StreamRegistry", () => {
  test("pulls base64 chunks no larger than 64 KiB and releases at EOF", async () => {
    const registry = new StreamRegistry()
    const bytes = new Uint8Array(MAX_STREAM_CHUNK_BYTES + 7).map((_, index) => index % 251)
    const descriptor = registry.add(
      new ReadableStream({
        start(controller) {
          controller.enqueue(bytes)
          controller.close()
        },
      }),
      bytes.byteLength,
    )
    const first = await registry.read({ streamID: descriptor.streamID })
    const second = await registry.read({ streamID: descriptor.streamID })
    const third = await registry.read({ streamID: descriptor.streamID })
    expect(Buffer.from(first.data, "base64").byteLength).toBe(MAX_STREAM_CHUNK_BYTES)
    expect(Buffer.from(second.data, "base64").byteLength).toBe(7)
    expect(third).toEqual({ data: "", eof: true })
    expect(registry.size).toBe(0)
  })

  test("cancels registered readers idempotently", async () => {
    let reason: unknown
    const registry = new StreamRegistry()
    const descriptor = registry.add(
      new ReadableStream({
        cancel(value) {
          reason = value
        },
      }),
    )
    expect(await registry.cancel({ streamID: descriptor.streamID, reason: "closed" })).toEqual({ cancelled: true })
    expect(await registry.cancel({ streamID: descriptor.streamID })).toEqual({ cancelled: false })
    expect(reason).toBe("closed")
  })

  test("cancellation interrupts a pending read", async () => {
    const registry = new StreamRegistry()
    const descriptor = registry.add(new ReadableStream({ pull() {} }))
    const read = registry.read({ streamID: descriptor.streamID })
    await Promise.resolve()
    expect(await registry.cancel({ streamID: descriptor.streamID, reason: "stop" })).toEqual({ cancelled: true })
    expect(await read).toEqual({ data: "", eof: true })
  })

  test("turns a remote descriptor into a pull-based ReadableStream", async () => {
    const registry = new StreamRegistry("backend")
    const descriptor = registry.add(new Blob(["hello world"]).stream() as ReadableStream<Uint8Array>)
    const calls: string[] = []
    const stream = remoteReadable(
      {
        async request<Result>(method: string, params: unknown) {
          calls.push(method)
          const input = params as { streamID: string; maxBytes?: number }
          if (method.endsWith(".read")) return (await registry.read(input)) as Result
          return (await registry.cancel(input)) as Result
        },
      },
      "backend",
      descriptor,
      { instanceID: "instance-1" },
    )
    expect(await new Response(stream).text()).toBe("hello world")
    expect(calls).toEqual(["backend.stream.read", "backend.stream.read"])
  })
})

function peerPair(options: { maxFrameBytes?: number } = {}) {
  let host: RpcPeer
  let backend: RpcPeer
  host = new RpcPeer(
    { write: (data) => deliver(backend, data, 7) },
    { idPrefix: "host", maxFrameBytes: options.maxFrameBytes },
  )
  backend = new RpcPeer(
    { write: (data) => deliver(host, data, 11) },
    { idPrefix: "backend", maxFrameBytes: options.maxFrameBytes },
  )
  return { host, backend }
}

async function deliver(peer: RpcPeer, data: Uint8Array, size: number) {
  for (let offset = 0; offset < data.byteLength; offset += size) {
    peer.receive(data.subarray(offset, Math.min(offset + size, data.byteLength)))
    await Promise.resolve()
  }
}

function hostResult(input: { value: string }) {
  return { value: `${input.value}:backend` }
}
