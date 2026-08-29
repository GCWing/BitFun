import path from "node:path"
import type { RpcConnection, StreamBridge } from "./backend"
import { ExtensionHostError } from "./errors"
import { ExtensionHost } from "./host"
import { logError, logEvent } from "./log"
import { createGateway } from "./gateway"
import { prepareBunPlugins } from "./bun-loader"
import {
  BackendMethodSchemas,
  DEFAULT_MAX_FRAME_BYTES,
  HOST_CAPABILITIES,
  OPENCODE_VERSION,
  PROTOCOL_VERSION,
  type BackendMethod,
} from "./protocol"
import { connectRpcPeer } from "./rpc"
import { registerHostMethods } from "./service"
import { remoteReadable, StreamRegistry } from "./streams"
import { registerStreamMethods } from "./stream-methods"
import { requireLoopbackAddress } from "./loopback"

await main()

async function main() {
  logEvent("startup.begin", { runtime: "bun" })
  configureLoopbackProxyBypass()
  const address = requiredEnvironment("OPENCODE_EXTENSION_HOST_RPC_ADDRESS")
  const token = requiredEnvironment("OPENCODE_EXTENSION_HOST_RPC_TOKEN")
  requireLoopbackAddress(address)
  let handshakeAccepted = false
  const peer = await connectRpcPeer(address, {
    idPrefix: "host",
    onRequest(method) {
      if (handshakeAccepted) return
      throw new ExtensionHostError(-32601, `Method not found: ${method}`)
    },
    onError(error) {
      logError("rpc.failure", error, { runtime: "bun" })
    },
  })
  logEvent("startup.rpc_connected", { runtime: "bun", address })
  const backend = protocolConnection(peer)
  const registry = new StreamRegistry("host")
  const owners = new Map<string, string>()
  const deferred = Promise.withResolvers<ExtensionHost>()
  void deferred.promise.catch(() => {})
  const streams: StreamBridge = {
    register(instanceID, stream, length) {
      const descriptor = registry.register(stream, length)
      owners.set(descriptor.streamID, instanceID)
      return descriptor
    },
    remote(methodPrefix, instanceID, descriptor) {
      return remoteReadable(backend, methodPrefix, descriptor, { instanceID })
    },
    async cancel(instanceID, descriptor) {
      if (owners.get(descriptor.streamID) !== instanceID) return
      owners.delete(descriptor.streamID)
      await registry.cancel({ streamID: descriptor.streamID, reason: "Stream owner released it" })
    },
    async cancelAll(instanceID) {
      await Promise.all(
        Array.from(owners, ([streamID, owner]) => {
          if (owner !== instanceID) return Promise.resolve()
          owners.delete(streamID)
          return registry.cancel({ streamID, reason: `Instance ${instanceID} closed` }).then(() => {})
        }),
      )
    },
    async cancelRemote(instanceID, descriptor, reason) {
      await backend.request("backend.stream.cancel", {
        instanceID,
        streamID: descriptor.streamID,
        ...(reason ? { reason } : {}),
      })
    },
  }

  let host: ExtensionHost | undefined
  registerStreamMethods(peer, registry, owners)
  registerHostMethods({
    peer,
    host: deferred.promise,
    shutdown() {
      void peer.flushAndClose().catch((error) => logError("shutdown.rpc_close_failed", error, { runtime: "bun" }))
    },
  })
  try {
    const handshakeMethod = BackendMethodSchemas["backend.handshake"]
    const handshake = handshakeMethod.result.parse(
      await peer.request(
        "backend.handshake",
        handshakeMethod.params.parse({
          token,
          protocolVersion: PROTOCOL_VERSION,
          opencodeVersion: OPENCODE_VERSION,
          maxFrameBytes: DEFAULT_MAX_FRAME_BYTES,
          capabilities: [...HOST_CAPABILITIES],
        }),
        {
          onResult(value) {
            handshakeMethod.result.parse(value)
            handshakeAccepted = true
          },
        },
      ),
    )
    if (!path.isAbsolute(handshake.cacheDirectory)) {
      throw new ExtensionHostError(-32001, "backend.handshake returned a relative cacheDirectory", {
        kind: "invalid_handshake",
        cacheDirectory: handshake.cacheDirectory,
      })
    }
    peer.setMaxFrameBytes(handshake.maxFrameBytes)
    logEvent("startup.handshake_complete", {
      runtime: "bun",
      max_frame_bytes: handshake.maxFrameBytes,
    })
    host = new ExtensionHost({
      rpc: backend,
      streams,
      cacheDirectory: handshake.cacheDirectory,
      gatewayFactory: createGateway,
      preparePlugins: prepareBunPlugins,
      shell: Bun.$,
    })
    deferred.resolve(host)
    logEvent("startup.ready", { runtime: "bun" })
    await peer.closed
    logEvent("rpc.closed", { runtime: "bun", failed: peer.closeError !== undefined })
    if (peer.closeError) throw peer.closeError
  } catch (error) {
    logError("startup.failed", error, { runtime: "bun" })
    deferred.reject(error)
    peer.close(error instanceof Error ? error : new Error(String(error)))
    throw error
  } finally {
    await host?.shutdown().catch((error) => logError("shutdown.failed", error, { runtime: "bun" }))
    await registry.cancelAll("Extension host connection closed")
    owners.clear()
    logEvent("shutdown.complete", { runtime: "bun" })
  }
}

function protocolConnection(peer: Awaited<ReturnType<typeof connectRpcPeer>>): RpcConnection {
  return {
    async request<Result>(method: string, params: unknown, options?: { signal?: AbortSignal }) {
      const definition = BackendMethodSchemas[method as BackendMethod]
      if (!definition) throw new TypeError(`Unknown backend method ${method}`)
      const validated = definition.params.parse(params)
      return definition.result.parse(await peer.request(method, validated, options)) as Result
    },
    notify(method: string, params: unknown) {
      const definition = BackendMethodSchemas[method as BackendMethod]
      if (!definition) throw new TypeError(`Unknown backend method ${method}`)
      return peer.notify(method, definition.params.parse(params))
    },
  }
}

function requiredEnvironment(name: string) {
  const value = Bun.env[name]
  if (value) return value
  throw new Error(`Missing required environment variable ${name}`)
}

function configureLoopbackProxyBypass() {
  for (const name of ["NO_PROXY", "no_proxy"] as const) {
    const values = new Set(
      (Bun.env[name] ?? "")
        .split(",")
        .map((value) => value.trim())
        .filter(Boolean),
    )
    values.add("127.0.0.1")
    values.add("localhost")
    values.add("::1")
    Bun.env[name] = Array.from(values).join(",")
  }
}
