import type { RpcConnection } from "./backend"
import { ExtensionHostError } from "./errors"
import { ExtensionHost, type InstanceOpenInput, type PluginsPrepareInput } from "./host"
import { logEvent, setLogLevel } from "./log"
import { HostMethodSchemas, type HostMethod } from "./protocol"
import type { WireValue } from "./wire"

export type HandlerPeer = RpcConnection & {
  handle(method: string, handler: (params: unknown) => unknown | Promise<unknown>): void
}

export function registerHostMethods(input: {
  peer: HandlerPeer
  host: ExtensionHost | Promise<ExtensionHost>
  shutdown(): void
}) {
  const host = () => Promise.resolve(input.host)
  const register = (method: HostMethod, handler: (params: unknown) => unknown | Promise<unknown>) => {
    input.peer.handle(method, async (params) => {
      const parsed = HostMethodSchemas[method].params.safeParse(params)
      if (!parsed.success) {
        throw new ExtensionHostError(-32602, `Invalid parameters for ${method}`, {
          kind: "invalid_params",
          method,
          issues: parsed.error.issues,
        })
      }
      return HostMethodSchemas[method].result.parse(await handler(parsed.data))
    })
  }

  register("host.plugins.prepare", async (params) => (await host()).prepare(params as PluginsPrepareInput))
  register("host.instance.open", async (params) => (await host()).open(params as InstanceOpenInput))
  register("host.instance.close", async (params) => (await host()).close(params as { instanceID: string }))
  register("host.log.setLevel", (params) => {
    const value = HostMethodSchemas["host.log.setLevel"].params.parse(params)
    return { level: setLogLevel(value.level) }
  })
  register("host.hook.call", async (params) =>
    (await host()).callHook(
      (() => {
        const value = params as {
          instanceID: string
          generationKey?: string
          revision?: string
          hook: string
          input: WireValue
          output: WireValue
        }
        return { ...value, name: value.hook }
      })(),
    ),
  )
  register("host.event.emit", async (params) =>
    (await host()).emitEvent(params as { instanceID: string; event: WireValue }),
  )
  register("host.tool.execute", async (params) =>
    (await host()).executeTool(params as Parameters<ExtensionHost["executeTool"]>[0]),
  )
  register("host.tool.cancel", async (params) =>
    (await host()).cancelTool(params as Parameters<ExtensionHost["cancelTool"]>[0]),
  )
  register("host.auth.prompt.evaluate", async (params) =>
    (await host()).evaluateAuthPrompt(params as Parameters<ExtensionHost["evaluateAuthPrompt"]>[0]),
  )
  register("host.auth.authorize", async (params) =>
    (await host()).authorize(params as Parameters<ExtensionHost["authorize"]>[0]),
  )
  register("host.auth.callback", async (params) =>
    (await host()).authCallback(params as Parameters<ExtensionHost["authCallback"]>[0]),
  )
  register("host.auth.flow.cancel", async (params) =>
    (await host()).cancelAuthFlow(params as Parameters<ExtensionHost["cancelAuthFlow"]>[0]),
  )
  register("host.auth.loader", async (params) =>
    (await host()).loadAuth(params as Parameters<ExtensionHost["loadAuth"]>[0]),
  )
  register("host.auth.fetch", async (params) =>
    (await host()).authFetch(params as Parameters<ExtensionHost["authFetch"]>[0]),
  )
  register("host.auth.fetch.cancel", async (params) =>
    (await host()).cancelAuthFetch(params as Parameters<ExtensionHost["cancelAuthFetch"]>[0]),
  )
  register("host.auth.fetch.release", async (params) =>
    (await host()).releaseAuthFetch(params as Parameters<ExtensionHost["releaseAuthFetch"]>[0]),
  )
  register("host.provider.models", async (params) =>
    (await host()).providerModels(params as Parameters<ExtensionHost["providerModels"]>[0]),
  )
  register("host.workspace.configure", async (params) =>
    (await host()).workspaceConfigure(params as Parameters<ExtensionHost["workspaceConfigure"]>[0]),
  )
  register("host.workspace.create", async (params) =>
    (await host()).workspaceCreate(params as Parameters<ExtensionHost["workspaceCreate"]>[0]),
  )
  register("host.workspace.remove", async (params) =>
    (await host()).workspaceRemove(params as Parameters<ExtensionHost["workspaceRemove"]>[0]),
  )
  register("host.workspace.target", async (params) =>
    (await host()).workspaceTarget(params as Parameters<ExtensionHost["workspaceTarget"]>[0]),
  )
  register("host.shutdown", async () => {
    logEvent("shutdown.requested")
    const result = await (await host()).shutdown()
    logEvent("shutdown.instances_closed")
    setTimeout(() => input.shutdown(), 0)
    return result
  })
}
