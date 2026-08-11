import { afterAll, beforeAll, describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { createConnection } from "node:net"
import path from "node:path"
import { tmpdir } from "node:os"
import type { RpcConnection, StreamBridge, StreamDescriptor } from "../src/backend"
import { createGateway } from "../src/gateway"
import { ExtensionHost } from "../src/host"
import { preparePlugins } from "../src/loader"
import { StreamRegistry, remoteReadable } from "../src/streams"

const temporaryDirectories: string[] = []
const noProxy = process.env.NO_PROXY
const noProxyLowercase = process.env.no_proxy

beforeAll(() => {
  process.env.NO_PROXY = [process.env.NO_PROXY, "127.0.0.1", "localhost"].filter(Boolean).join(",")
  process.env.no_proxy = [process.env.no_proxy, "127.0.0.1", "localhost"].filter(Boolean).join(",")
})

afterAll(async () => {
  await Promise.all(temporaryDirectories.map((directory) => rm(directory, { recursive: true, force: true })))
  restoreEnvironment("NO_PROXY", noProxy)
  restoreEnvironment("no_proxy", noProxyLowercase)
})

describe("per-instance HTTP gateway", () => {
  test("forwards method, path, headers, and streaming request and response bodies", async () => {
    const streams = new TestStreams()
    const requests: BackendRequest[] = []
    const rpc = createRpc(async (method, params) => {
      expect(method).toBe("backend.http.request")
      const request = params as BackendRequest
      requests.push(request)
      expect(await streams.readHost(request.body)).toBe("request-one-request-two")
      return {
        status: 207,
        headers: [
          ["content-type", "text/plain"],
          ["x-backend", "forwarded"],
        ],
        body: streams.addBackend(
          new ReadableStream({
            start(controller) {
              controller.enqueue(Buffer.from("response-one-"))
              controller.enqueue(Buffer.from("response-two"))
              controller.close()
            },
          }),
          25,
        ),
      }
    })
    const gateway = createGateway({ instanceID: "instance-http", rpc, streams })

    try {
      const body = new ReadableStream({
        start(controller) {
          controller.enqueue(Buffer.from("request-one-"))
          controller.enqueue(Buffer.from("request-two"))
          controller.close()
        },
      })
      const response = await fetch(new URL("/api/items?limit=2&tag=a", gateway.url), {
        method: "POST",
        headers: {
          "content-length": "23",
          "content-type": "application/octet-stream",
          "x-plugin": "fixture",
        },
        body,
        duplex: "half",
      })

      expect(response.status).toBe(207)
      expect(response.headers.get("x-backend")).toBe("forwarded")
      expect(await response.text()).toBe("response-one-response-two")
      expect(requests).toHaveLength(1)
      expect(requests[0]).toMatchObject({
        instanceID: "instance-http",
        method: "POST",
        path: "/api/items?limit=2&tag=a",
      })
      expect(requests[0]?.requestID).toMatch(/^[0-9a-f-]{36}$/)
      expect(new Headers(requests[0]?.headers).get("x-plugin")).toBe("fixture")
      expect(requests[0]?.body?.length).toBe(23)
      expect(streams.backendReadCount).toBeGreaterThanOrEqual(2)
    } finally {
      await gateway.close()
    }
  })

  test("streams SSE incrementally and cancels the backend body when the client stops reading", async () => {
    const streams = new TestStreams()
    const next = Promise.withResolvers<void>()
    const never = Promise.withResolvers<void>()
    const cancelled = Promise.withResolvers<void>()
    streams.onBackendCancel = () => cancelled.resolve()
    const rpc = createRpc(async () => ({
      status: 200,
      headers: [["content-type", "text/event-stream"]],
      body: streams.addBackend(
        new ReadableStream({
          async pull(controller) {
            if (!streams.backendProduced) {
              streams.backendProduced = 1
              controller.enqueue(Buffer.from("data: first\n\n"))
              return
            }
            if (streams.backendProduced === 1) {
              await next.promise
              streams.backendProduced = 2
              controller.enqueue(Buffer.alloc(256 * 1024, 120))
              return
            }
            await never.promise
          },
        }),
      ),
    }))
    const gateway = createGateway({ instanceID: "instance-sse", rpc, streams })

    try {
      const controller = new AbortController()
      const response = await fetch(new URL("/event", gateway.url), { signal: controller.signal })
      expect(response.headers.get("content-type")).toBe("text/event-stream")
      const reader = response.body!.getReader()
      expect(Buffer.from((await reader.read()).value!).toString()).toBe("data: first\n\n")
      expect(streams.backendProduced).toBe(1)
      controller.abort("fixture finished")
      next.resolve()
      await Promise.race([
        cancelled.promise,
        Bun.sleep(1_000).then(() => {
          throw new Error("Backend stream cancellation was not forwarded")
        }),
      ])
    } finally {
      next.resolve()
      await gateway.close()
    }
  })

  test("rejects WebSocket upgrades without forwarding them to Rust", async () => {
    const streams = new TestStreams()
    const methods: string[] = []
    const rpc = createRpc(async (method) => {
      methods.push(method)
      throw new Error("WebSocket request should not be forwarded")
    })
    const gateway = createGateway({ instanceID: "instance-websocket", rpc, streams })

    try {
      const response = await rawHttp(
        gateway.url,
        [
          "GET /socket HTTP/1.1",
          `Host: ${gateway.url.host}`,
          "Connection: Upgrade",
          "Upgrade: websocket",
          "Sec-WebSocket-Version: 13",
          "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==",
          "",
          "",
        ].join("\r\n"),
      )

      expect(response).toContain(" 426 ")
      expect(response).toContain("WebSocket forwarding is not supported")
      expect(methods).toEqual([])
    } finally {
      await gateway.close()
    }
  })

  test("releases request bodies when backend forwarding fails", async () => {
    const streams = new TestStreams()
    const gateway = createGateway({
      instanceID: "instance-failure",
      streams,
      rpc: createRpc(async () => {
        throw new Error("backend unavailable")
      }),
    })

    try {
      const response = await fetch(new URL("/failure", gateway.url), { method: "POST", body: "request body" })
      expect(response.status).toBe(502)
      expect(streams.hostSize).toBe(0)
    } finally {
      await gateway.close()
    }
  })
})

describe("plugin injected API", () => {
  test("supports the SDK, raw serverUrl, Bun shell, and workspace registration during initialization", async () => {
    const directory = await temporaryDirectory()
    const streams = new TestStreams()
    const paths: string[] = []
    const rpc = createRpc(async (method, params) => {
      expect(method).toBe("backend.http.request")
      const request = params as BackendRequest
      paths.push(request.path)
      if (request.path.startsWith("/project/current")) {
        return jsonResponse(streams, {
          id: "project-injected",
          worktree: directory,
          vcs: "git",
          time: { created: 1, updated: 2 },
        })
      }
      if (request.path === "/raw?fixture=1") {
        return textResponse(streams, "raw-gateway-ok")
      }
      throw new Error(`Unexpected gateway path ${request.path}`)
    })
    const host = new ExtensionHost({
      rpc,
      streams,
      cacheDirectory: path.join(directory, "cache"),
      gatewayFactory: createGateway,
      preparePlugins,
      shell: Bun.$,
    })
    const fixture = path.join(import.meta.dir, "fixtures/gateway/injected.ts")

    try {
      const opened = await host.open({
        instanceID: "instance-injected",
        project: { id: "project-injected" },
        directory,
        worktree: directory,
        config: {},
        plugins: [{ spec: fixture }],
      })

      expect(opened.diagnostics).toEqual([])
      expect(paths).toHaveLength(2)
      expect(paths[0]).toBe(`/project/current?directory=${encodeURIComponent(directory)}`)
      expect(paths[1]).toBe("/raw?fixture=1")
      expect(opened.config).toMatchObject({
        injectedFixture: {
          projectID: "project-injected",
          raw: "raw-gateway-ok",
          shell: "injected-shell",
          serverURL: opened.gatewayURL,
          directory,
          worktree: directory,
        },
      })
      expect(opened.workspaces).toEqual([
        expect.objectContaining({
          type: "fixture-remote",
          name: "Fixture remote",
          description: "Workspace registered by the injected API fixture",
        }),
      ])

      const registrationID = opened.workspaces[0]!.registrationID
      const config = {
        id: "workspace-1",
        type: "fixture-remote",
        name: "demo",
        branch: null,
        directory: null,
        extra: null,
        projectID: "project-injected",
      }
      expect(await host.workspaceConfigure({ instanceID: opened.instanceID, registrationID, config })).toEqual({
        config: { ...config, name: "demo-configured" },
      })
      expect(await host.workspaceTarget({ instanceID: opened.instanceID, registrationID, config })).toEqual({
        target: {
          type: "remote",
          url: "https://workspace.example.test/root",
          headers: [
            ["x-fixture", "yes"],
            ["x-second", "two"],
          ],
        },
      })
    } finally {
      await host.shutdown()
    }
  })
})

type BackendRequest = {
  instanceID: string
  requestID: string
  method: string
  path: string
  headers: Array<[string, string]>
  body?: StreamDescriptor
}

class TestStreams implements StreamBridge {
  readonly #host = new StreamRegistry("test-host")
  readonly #backend = new StreamRegistry("test-backend")
  backendReadCount = 0
  backendProduced = 0
  backendCancelReasons: string[] = []
  onBackendCancel?: () => void

  get hostSize() {
    return this.#host.size
  }

  register(_instanceID: string, stream: ReadableStream<Uint8Array>, length?: number) {
    return this.#host.add(stream, length)
  }

  remote(methodPrefix: "backend" | "host", instanceID: string, descriptor: StreamDescriptor) {
    expect(methodPrefix).toBe("backend")
    return remoteReadable(
      {
        request: async <Result>(method: string, params: unknown) => {
          const input = params as { streamID: string; maxBytes?: number; reason?: string }
          if (method === "backend.stream.read") {
            this.backendReadCount += 1
            return this.#backend.read(input) as Promise<Result>
          }
          if (method === "backend.stream.cancel") {
            if (input.reason) this.backendCancelReasons.push(input.reason)
            const result = await this.#backend.cancel(input)
            this.onBackendCancel?.()
            return result as Result
          }
          throw new Error(`Unexpected stream method ${method}`)
        },
      },
      "backend",
      descriptor,
      { instanceID },
    )
  }

  async cancel(_instanceID: string, descriptor: StreamDescriptor) {
    await this.#host.cancel(descriptor)
  }

  async cancelAll(_instanceID: string) {
    await this.#host.cancelAll()
  }

  addBackend(stream: ReadableStream<Uint8Array>, length?: number) {
    return this.#backend.add(stream, length)
  }

  async readHost(descriptor?: StreamDescriptor) {
    if (!descriptor) return ""
    const chunks: Uint8Array[] = []
    while (true) {
      const result = await this.#host.read({ streamID: descriptor.streamID })
      if (result.data) chunks.push(Buffer.from(result.data, "base64"))
      if (result.eof) return Buffer.concat(chunks).toString()
    }
  }
}

function createRpc(request: (method: string, params: unknown) => Promise<unknown>): RpcConnection {
  return {
    request<Result>(method: string, params: unknown) {
      return request(method, params) as Promise<Result>
    },
    notify() {},
  }
}

function jsonResponse(streams: TestStreams, value: unknown) {
  const body = JSON.stringify(value)
  return {
    status: 200,
    headers: [
      ["content-type", "application/json"],
      ["content-length", String(Buffer.byteLength(body))],
    ],
    body: streams.addBackend(new Blob([body]).stream(), Buffer.byteLength(body)),
  }
}

function textResponse(streams: TestStreams, value: string) {
  return {
    status: 200,
    headers: [
      ["content-type", "text/plain"],
      ["content-length", String(Buffer.byteLength(value))],
    ],
    body: streams.addBackend(new Blob([value]).stream(), Buffer.byteLength(value)),
  }
}

async function temporaryDirectory() {
  const directory = await mkdtemp(path.join(tmpdir(), "opencode-extension-host-gateway-"))
  temporaryDirectories.push(directory)
  return directory
}

function rawHttp(url: URL, request: string) {
  const deferred = Promise.withResolvers<string>()
  const chunks: Buffer[] = []
  const socket = createConnection({ host: url.hostname, port: Number(url.port) })
  socket.on("connect", () => socket.write(request))
  socket.on("data", (chunk) => {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk))
    const response = Buffer.concat(chunks)
    const boundary = response.indexOf("\r\n\r\n")
    if (boundary < 0) return
    const match = response
      .subarray(0, boundary)
      .toString()
      .match(/content-length:\s*(\d+)/i)
    if (!match || response.byteLength < boundary + 4 + Number(match[1])) return
    socket.destroy()
    deferred.resolve(response.toString())
  })
  socket.on("error", deferred.reject)
  socket.on("end", () => deferred.resolve(Buffer.concat(chunks).toString()))
  return deferred.promise
}

function restoreEnvironment(key: string, value?: string) {
  if (value === undefined) {
    delete process.env[key]
    return
  }
  process.env[key] = value
}
