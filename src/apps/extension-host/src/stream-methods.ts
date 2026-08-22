import { ExtensionHostError } from "./errors"
import { HostMethodSchemas } from "./protocol"
import { StreamRegistry } from "./streams"

type StreamMethodPeer = {
  handle(method: string, handler: (value: unknown) => unknown | Promise<unknown>): unknown
}

export function registerStreamMethods(
  peer: StreamMethodPeer,
  registry: StreamRegistry,
  owners: Map<string, string>,
) {
  peer.handle("host.stream.read", async (value) => {
    const input = parseStreamParams("host.stream.read", value)
    requireStreamOwner(owners, input.instanceID, input.streamID)
    try {
      const result = await registry.read(input)
      if (result.eof) owners.delete(input.streamID)
      return HostMethodSchemas["host.stream.read"].result.parse(result)
    } catch (error) {
      owners.delete(input.streamID)
      throw error
    }
  })
  peer.handle("host.stream.cancel", async (value) => {
    const input = parseStreamParams("host.stream.cancel", value)
    if (!owners.has(input.streamID)) return { cancelled: false }
    requireStreamOwner(owners, input.instanceID, input.streamID)
    owners.delete(input.streamID)
    return HostMethodSchemas["host.stream.cancel"].result.parse(await registry.cancel(input))
  })
}

function parseStreamParams(method: "host.stream.read" | "host.stream.cancel", value: unknown) {
  const parsed = HostMethodSchemas[method].params.safeParse(value)
  if (parsed.success) return parsed.data
  throw new ExtensionHostError(-32602, `Invalid parameters for ${method}`, {
    kind: "invalid_params",
    method,
    issues: parsed.error.issues,
  })
}

function requireStreamOwner(owners: Map<string, string>, instanceID: string, streamID: string) {
  if (owners.get(streamID) === instanceID) return
  throw new ExtensionHostError(-32002, `Unknown stream ${streamID} for instance ${instanceID}`, {
    kind: "missing_handle",
    type: "stream",
    id: streamID,
    instanceID,
  })
}
