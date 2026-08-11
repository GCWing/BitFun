import type { RpcConnection, StreamBridge, StreamDescriptor } from "./backend"
import { publishDiagnostic } from "./backend"

type GatewayResponse = {
  status: number
  statusText?: string
  headers?: Array<[string, string]> | Record<string, string>
  body?: StreamDescriptor
}

export type Gateway = {
  url: URL
  close(): Promise<void>
}

export type GatewayFactory = (
  input: { instanceID: string; rpc: RpcConnection; streams: StreamBridge },
) => Gateway | Promise<Gateway>

export function createGateway(input: { instanceID: string; rpc: RpcConnection; streams: StreamBridge }): Gateway {
  const active = new Set<StreamDescriptor>()
  const server = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    async fetch(request) {
      if (request.headers.get("upgrade")) {
        return Response.json(
          { error: "WebSocket forwarding is not supported by this extension host" },
          { status: 426, headers: { upgrade: "close" } },
        )
      }

      const length = request.headers.get("content-length")
      const body = request.body
        ? input.streams.register(
            input.instanceID,
            request.body,
            length && Number.isSafeInteger(Number(length)) ? Number(length) : undefined,
          )
        : undefined
      if (body) active.add(body)

      try {
        const url = new URL(request.url)
        const result = await input.rpc.request<GatewayResponse>(
          "backend.http.request",
          {
            instanceID: input.instanceID,
            requestID: crypto.randomUUID(),
            method: request.method,
            path: `${url.pathname}${url.search}`,
            headers: Array.from(request.headers.entries()),
            body,
          },
          { signal: request.signal },
        )
        validateResponse(result)

        const headers = new Headers(result.headers)
        if (!result.body) {
          return new Response(null, { status: result.status, statusText: result.statusText, headers })
        }

        const stream = input.streams.remote("backend", input.instanceID, result.body)
        return new Response(stream, { status: result.status, statusText: result.statusText, headers })
      } catch (error) {
        if (body) await input.streams.cancel(input.instanceID, body).catch(() => {})
        await publishDiagnostic(input.rpc, {
          level: "error",
          message: "Failed to forward plugin HTTP request",
          instanceID: input.instanceID,
          operation: "backend.http.request",
          error: errorInfo(error),
        }).catch(() => {})
        return Response.json({ error: "Extension backend request failed" }, { status: 502 })
      } finally {
        if (body) active.delete(body)
      }
    },
  })

  return {
    url: server.url,
    async close() {
      await Promise.all(
        Array.from(active, (descriptor) => input.streams.cancel(input.instanceID, descriptor).catch(() => {})),
      )
      active.clear()
      await server.stop(true)
    },
  }
}

function validateResponse(value: GatewayResponse) {
  if (!value || typeof value !== "object") throw new TypeError("backend.http.request returned a non-object")
  if (!Number.isInteger(value.status) || value.status < 100 || value.status > 599) {
    throw new TypeError("backend.http.request returned an invalid status")
  }
  if (!value.body) return
  if (typeof value.body.streamID !== "string" || !value.body.streamID) {
    throw new TypeError("backend.http.request returned an invalid body stream")
  }
}

function errorInfo(error: unknown) {
  if (!(error instanceof Error)) return { message: String(error) }
  return {
    name: error.name,
    message: error.message,
    stack: error.stack,
    cause: error.cause,
  }
}
