import path from "node:path"
import { realpath } from "node:fs/promises"
import type {
  AuthHook,
  AuthOAuthResult,
  Hooks,
  PluginInput,
  PluginOptions,
  ProviderHook,
  ToolDefinition,
  WorkspaceAdapter,
  WorkspaceInfo,
  WorkspaceTarget,
} from "@opencode-ai/plugin"
import { createOpencodeClient } from "@opencode-ai/sdk"
import type { Auth, Provider } from "@opencode-ai/sdk/v2"
import type { RpcConnection, StreamBridge, StreamDescriptor } from "./backend"
import { publishDiagnostic } from "./backend"
import { ExtensionHostError, errorData } from "./errors"
import type { Gateway, GatewayFactory } from "./gateway"
import { logError, logEvent } from "./log"
import {
  type LoadPluginsInput,
  type LoadedPlugin,
  type LoaderDiagnostic,
  type PluginDeclaration,
  loadPreparedPlugins,
  type PreparePluginsResult,
} from "./loader"
import { HostMethodSchemas } from "./protocol"
import { toolParametersToJsonSchema, validateToolArguments } from "./tool-schema"
import { cloneWireValue, type WireValue } from "./wire"

const OPENCODE_VERSION = "1.17.18"
const GENERIC_HOOKS = [
  "chat.message",
  "chat.params",
  "chat.headers",
  "permission.ask",
  "command.execute.before",
  "tool.execute.before",
  "shell.env",
  "tool.execute.after",
  "experimental.chat.messages.transform",
  "experimental.chat.system.transform",
  "experimental.provider.small_model",
  "experimental.session.compacting",
  "experimental.compaction.autocontinue",
  "experimental.text.complete",
  "tool.definition",
] as const

type PluginMeta = {
  id?: string
  spec: string
  entry: string
  index: number
}

type RuntimeDiagnostic = {
  level: "error"
  stage: "runtime"
  spec: string
  pluginID?: string
  message: string
  error?: {
    name?: string
    message: string
    stack?: string
    cause?: unknown
  }
}

type HostDiagnostic = LoaderDiagnostic | RuntimeDiagnostic

type RetainedHooks = {
  plugin: PluginMeta
  hooks: Hooks
}

type ToolRegistration = {
  registrationID: string
  plugin: PluginMeta
  id: string
  definition: ToolDefinition
  parameters: WireValue
}

type AuthRegistration = {
  plugin: PluginMeta
  hook: AuthHook
}

type ProviderRegistration = {
  plugin: PluginMeta
  hook: ProviderHook
}

type WorkspaceRegistration = {
  registrationID: string
  plugin: PluginMeta
  type: string
  adapter: WorkspaceAdapter
}

type OAuthFlow = {
  plugin: PluginMeta
  method: AuthOAuthResult["method"]
  callback: AuthOAuthResult["callback"]
}

type AuthFetch = {
  plugin: PluginMeta
  provider: string
  fetch: typeof fetch
}

type ActiveAuthFetch = {
  controller: AbortController
  body?: ReadableStream<Uint8Array>
  descriptor?: StreamDescriptor
}

type Instance = {
  id: string
  canonicalDirectory: string
  directory: string
  worktree: string
  status: "opening" | "open" | "closing"
  gateway: Gateway
  hooks: RetainedHooks[]
  tools: Map<string, ToolRegistration>
  auth: Map<string, AuthRegistration>
  providers: Map<string, ProviderRegistration>
  workspaces: Map<string, WorkspaceRegistration>
  flows: Map<string, OAuthFlow>
  fetches: Map<string, AuthFetch>
  activeTools: Map<string, AbortController>
  activeFetches: Map<string, ActiveAuthFetch>
  openDone: Promise<void>
  finishOpen(): void
  closePromise?: Promise<void>
  disposed: Set<RetainedHooks>
  counter: number
}

export type InstanceOpenInput = {
  instanceID: string
  project: WireValue
  directory: string
  worktree: string
  config: WireValue
  plugins: PluginDeclaration[]
  configurationFingerprint?: string
}

export type PluginsPrepareInput = {
  plugins: PluginDeclaration[]
  configurationFingerprint?: string
  defaultBaseDirectory?: string
}

export class ExtensionHost {
  readonly #rpc: RpcConnection
  readonly #streams: StreamBridge
  readonly #cacheDirectory: string
  readonly #gatewayFactory: GatewayFactory
  readonly #preparePlugins: (input: LoadPluginsInput) => Promise<PreparePluginsResult>
  readonly #shell: PluginInput["$"]
  readonly #instances = new Map<string, Instance>()
  readonly #directories = new Map<string, string>()
  readonly #opening = new Map<string, Promise<void>>()
  readonly #preparations = new Map<string, Promise<PreparePluginsResult>>()
  readonly #cancelledOpenings = new Set<string>()
  #status: "running" | "closing" | "closed" = "running"
  #shutdownPromise?: Promise<void>

  constructor(input: {
    rpc: RpcConnection
    streams: StreamBridge
    cacheDirectory: string
    gatewayFactory: GatewayFactory
    preparePlugins: (input: LoadPluginsInput) => Promise<PreparePluginsResult>
    shell: PluginInput["$"]
  }) {
    this.#rpc = input.rpc
    this.#streams = input.streams
    this.#cacheDirectory = input.cacheDirectory
    this.#gatewayFactory = input.gatewayFactory
    this.#preparePlugins = input.preparePlugins
    this.#shell = input.shell
  }

  async prepare(input: PluginsPrepareInput) {
    this.#assertAccepting()
    const prepared = await this.#prepare({
      declarations: input.plugins,
      defaultBaseDirectory: input.defaultBaseDirectory,
      configurationFingerprint: input.configurationFingerprint,
    })
    return HostMethodSchemas["host.plugins.prepare"].result.parse({
      ...(input.configurationFingerprint
        ? { configurationFingerprint: input.configurationFingerprint }
        : {}),
      prepared: prepared.prepared.map((plugin) => ({
        spec: plugin.spec,
        source: plugin.source,
        target: plugin.target,
        entry: plugin.entry,
        cache: plugin.cache,
        ...(typeof plugin.package?.manifest.version === "string"
          ? { version: plugin.package.manifest.version }
          : {}),
      })),
      failed: prepared.diagnostics.map((diagnostic) => ({
        spec: diagnostic.spec,
        stage: diagnostic.stage,
        message: diagnostic.message,
      })),
      diagnostics: prepared.diagnostics.map(protocolDiagnostic),
    })
  }

  async open(input: InstanceOpenInput) {
    this.#assertAccepting()
    if (this.#instances.has(input.instanceID) || this.#opening.has(input.instanceID)) {
      throw new ExtensionHostError(-32002, `Instance ${input.instanceID} already exists`, {
        kind: "instance_exists",
        instanceID: input.instanceID,
      })
    }
    const operation = Promise.withResolvers<void>()
    this.#opening.set(input.instanceID, operation.promise)
    try {
      const canonicalDirectory = await realpath(path.resolve(input.directory)).catch(() =>
        path.resolve(input.directory),
      )
      this.#assertAccepting()
      if (this.#cancelledOpenings.has(input.instanceID)) {
        throw new ExtensionHostError(-32004, `Instance ${input.instanceID} was closed while opening`, {
          kind: "instance_closing",
          instanceID: input.instanceID,
        })
      }
      const owner = this.#directories.get(canonicalDirectory)
      if (owner) {
        throw new ExtensionHostError(-32002, `Directory ${canonicalDirectory} is already owned by ${owner}`, {
          kind: "directory_exists",
          instanceID: owner,
          directory: canonicalDirectory,
        })
      }

      const config = cloneWireValue(input.config, "config")
      const gateway = await this.#gatewayFactory({
        instanceID: input.instanceID,
        rpc: this.#rpc,
        streams: this.#streams,
      })
      const opened = Promise.withResolvers<void>()
      const instance: Instance = {
        id: input.instanceID,
        canonicalDirectory,
        directory: input.directory,
        worktree: input.worktree,
        status: "opening",
        gateway,
        hooks: [],
        tools: new Map(),
        auth: new Map(),
        providers: new Map(),
        workspaces: new Map(),
        flows: new Map(),
        fetches: new Map(),
        activeTools: new Map(),
        activeFetches: new Map(),
        openDone: opened.promise,
        finishOpen: opened.resolve,
        disposed: new Set(),
        counter: 0,
      }
      this.#instances.set(instance.id, instance)
      this.#directories.set(canonicalDirectory, instance.id)

      let failure: unknown
      const diagnostics: HostDiagnostic[] = []
      try {
        logEvent("plugin.activation.begin", {
          instance_id: instance.id,
          plugin_count: input.plugins.length,
          plugins: input.plugins.map(pluginDeclarationSpec),
        })
        const prepared = await this.#prepare({
          declarations: input.plugins,
          defaultBaseDirectory: input.directory,
          configurationFingerprint: input.configurationFingerprint,
        })
        const loaded = await loadPreparedPlugins(prepared)
        this.#assertOpening(instance)
        diagnostics.push(...loaded.diagnostics)

        const client = createOpencodeClient({ baseUrl: gateway.url.toString(), directory: input.directory })
        for (const plugin of loaded.loaded) {
          this.#assertOpening(instance)
          await this.#startPlugin(instance, plugin, input.project, client, diagnostics)
        }

        const activatedPlugins = instance.hooks.map(({ plugin }) => plugin.spec)
        logEvent("plugin.activation.complete", {
          instance_id: instance.id,
          configured_plugin_count: input.plugins.length,
          loaded_plugin_count: loaded.loaded.length,
          activated_plugin_count: activatedPlugins.length,
          plugins: activatedPlugins,
          diagnostic_count: diagnostics.length,
        })

        for (const retained of instance.hooks) {
          this.#assertOpening(instance)
          if (!retained.hooks.config) continue
          try {
            await Promise.resolve(retained.hooks.config(config as never))
          } catch (error) {
            const diagnostic = runtimeDiagnostic(retained.plugin, "config", error)
            diagnostics.push(diagnostic)
            await publishDiagnostic(this.#rpc, toPublishedDiagnostic(instance.id, diagnostic)).catch(() => {})
          }
          this.#assertOpening(instance)
        }

        this.#assertOpening(instance)
        this.#indexRegistrations(instance, diagnostics)
        this.#assertOpening(instance)
        const result = HostMethodSchemas["host.instance.open"].result.parse(openResult(instance, config, diagnostics))
        instance.status = "open"
        return result
      } catch (error) {
        logError("plugin.activation.failed", error, {
          instance_id: instance.id,
          configured_plugin_count: input.plugins.length,
          activated_plugin_count: instance.hooks.length,
          plugins: instance.hooks.map(({ plugin }) => plugin.spec),
        })
        failure = error
      } finally {
        instance.finishOpen()
      }

      await this.#beginClose(instance)
      throw failure
    } finally {
      if (this.#opening.get(input.instanceID) === operation.promise) this.#opening.delete(input.instanceID)
      this.#cancelledOpenings.delete(input.instanceID)
      operation.resolve()
    }
  }

  async close(input: { instanceID: string }): Promise<{ closed: boolean }> {
    const pending = this.#opening.get(input.instanceID)
    const instance = this.#instances.get(input.instanceID)
    if (!instance && pending) {
      this.#cancelledOpenings.add(input.instanceID)
      await pending
      return { closed: true }
    }
    if (!instance) return { closed: false }
    const first = !instance.closePromise
    await this.#beginClose(instance)
    return { closed: first }
  }

  async shutdown() {
    if (!this.#shutdownPromise) {
      this.#status = "closing"
      this.#shutdownPromise = (async () => {
        await Promise.all([
          ...Array.from(this.#instances.values(), (instance) => this.#beginClose(instance)),
          ...this.#opening.values(),
          ...this.#preparations.values(),
        ])
        await Promise.all(Array.from(this.#instances.values(), (instance) => this.#beginClose(instance)))
        this.#status = "closed"
      })()
    }
    await this.#shutdownPromise
    return { closed: true }
  }

  #prepare(input: {
    declarations: readonly PluginDeclaration[]
    defaultBaseDirectory?: string
    configurationFingerprint?: string
  }) {
    const key = preparationKey(input)
    const pending = this.#preparations.get(key)
    if (pending) {
      logEvent("plugin.prepare.waiting_existing", {
        configuration_fingerprint: input.configurationFingerprint,
        plugin_count: input.declarations.length,
        plugins: input.declarations.map(pluginDeclarationSpec),
      }, "debug")
      return pending
    }

    const startedAt = performance.now()
    logEvent("plugin.prepare.begin", {
      configuration_fingerprint: input.configurationFingerprint,
      plugin_count: input.declarations.length,
      plugins: input.declarations.map(pluginDeclarationSpec),
    })
    const operation = this.#preparePlugins({
      declarations: input.declarations,
      cacheDirectory: this.#cacheDirectory,
      defaultBaseDirectory: input.defaultBaseDirectory,
      compatibilityVersion: OPENCODE_VERSION,
    })
      .then((result) => {
        for (const diagnostic of result.diagnostics) {
          logEvent("plugin.prepare.failed", {
            configuration_fingerprint: input.configurationFingerprint,
            plugin: diagnostic.spec,
            stage: diagnostic.stage,
            error_message: diagnostic.message,
          }, "error")
        }
        logEvent("plugin.prepare.completed", {
          configuration_fingerprint: input.configurationFingerprint,
          configured_plugin_count: input.declarations.length,
          prepared_plugin_count: result.prepared.length,
          failed_plugin_count: result.diagnostics.length,
          plugins: result.prepared.map((plugin) => plugin.spec),
          duration_ms: Math.round(performance.now() - startedAt),
        })
        return result
      })
      .catch((error) => {
        logError("plugin.prepare.failed", error, {
          configuration_fingerprint: input.configurationFingerprint,
          plugin_count: input.declarations.length,
          plugins: input.declarations.map(pluginDeclarationSpec),
          duration_ms: Math.round(performance.now() - startedAt),
        })
        throw error
      })
    this.#preparations.set(key, operation)
    return operation
  }

  async callHook(input: { instanceID: string; name: string; input: WireValue; output: WireValue }) {
    const instance = this.#instance(input.instanceID)
    const hookInput = cloneWireValue(input.input, "input")
    const hookOutput = cloneWireValue(input.output, "output")

    for (const retained of instance.hooks) {
      const hook = Reflect.get(retained.hooks, input.name)
      if (typeof hook !== "function") continue
      try {
        await Promise.resolve(hook(hookInput, hookOutput))
      } catch (error) {
        throw pluginError(retained.plugin, input.name, error)
      }
    }

    return {
      input: cloneWireValue(hookInput, "input"),
      output: cloneWireValue(hookOutput, "output"),
    }
  }

  emitEvent(input: { instanceID: string; event: WireValue }) {
    const instance = this.#instance(input.instanceID)
    const event = cloneWireValue(input.event, "event")
    void (async () => {
      for (const retained of instance.hooks) {
        if (!retained.hooks.event) continue
        try {
          await retained.hooks.event({ event } as never)
        } catch (error) {
          await publishDiagnostic(this.#rpc, {
            level: "error",
            message: `Plugin ${retained.plugin.spec} event hook failed`,
            instanceID: instance.id,
            plugin: retained.plugin,
            operation: "event",
            error: errorData(error),
          }).catch(() => {})
        }
      }
    })()
    return { accepted: true }
  }

  async executeTool(input: {
    instanceID: string
    registrationID: string
    executionID: string
    args: WireValue
    context: {
      sessionID: string
      messageID: string
      agent: string
      callID?: string
    }
  }) {
    const instance = this.#instance(input.instanceID)
    const registration = findRegistration(instance.tools, input.registrationID)
    if (!registration) throw missingHandle("tool", input.registrationID)
    if (instance.activeTools.has(input.executionID)) {
      throw new ExtensionHostError(-32002, `Tool execution ${input.executionID} already exists`)
    }

    const controller = new AbortController()
    instance.activeTools.set(input.executionID, controller)
    try {
      const args = validateToolArguments(registration.definition.args, cloneWireValue(input.args, "args"))
      const result = await registration.definition.execute(args as never, {
        ...input.context,
        directory: instance.directory,
        worktree: instance.worktree,
        abort: controller.signal,
        metadata: (metadata) => {
          const pending = this.#rpc.notify("backend.tool.metadata", {
            instanceID: instance.id,
            executionID: input.executionID,
            ...(cloneWireValue(metadata, "metadata") as Record<string, WireValue>),
          })
          if (pending) void pending.catch(() => {})
        },
        ask: async (request) => {
          await this.#rpc.request(
            "backend.tool.ask",
            {
              instanceID: instance.id,
              executionID: input.executionID,
              ...(cloneWireValue(request, "request") as Record<string, WireValue>),
            },
            { signal: controller.signal },
          )
        },
      })
      return cloneWireValue(result, "result")
    } catch (error) {
      throw pluginError(registration.plugin, `tool:${registration.id}`, error)
    } finally {
      instance.activeTools.delete(input.executionID)
    }
  }

  cancelTool(input: { instanceID: string; executionID: string; reason?: string }) {
    const controller = this.#instance(input.instanceID).activeTools.get(input.executionID)
    if (!controller) return { cancelled: false }
    controller.abort(input.reason)
    return { cancelled: true }
  }

  evaluateAuthPrompt(input: {
    instanceID: string
    provider: string
    methodIndex: number
    promptIndex: number
    operation: "validate" | "condition"
    value?: string
    inputs: Record<string, string>
  }) {
    const registration = this.#auth(input.instanceID, input.provider)
    const method = registration.hook.methods[input.methodIndex]
    const prompt = method?.prompts?.[input.promptIndex]
    if (!method || !prompt) {
      throw missingHandle("auth prompt", `${input.provider}:${input.methodIndex}:${input.promptIndex}`)
    }

    if (input.operation === "validate") {
      const error =
        prompt.type === "text" && prompt.validate
          ? prompt.validate(input.value ?? input.inputs[prompt.key] ?? "")
          : undefined
      return { operation: "validate" as const, ...(error ? { error } : {}) }
    }

    const active = prompt.when
      ? prompt.when.op === "eq"
        ? input.inputs[prompt.when.key] === prompt.when.value
        : input.inputs[prompt.when.key] !== prompt.when.value
      : prompt.condition
        ? prompt.condition(input.inputs)
        : true
    return { operation: "condition" as const, active }
  }

  async authorize(input: {
    instanceID: string
    provider: string
    methodIndex: number
    inputs?: Record<string, string>
  }) {
    const instance = this.#instance(input.instanceID)
    const registration = this.#auth(input.instanceID, input.provider)
    const method = registration.hook.methods[input.methodIndex]
    if (!method) throw missingHandle("auth method", `${input.provider}:${input.methodIndex}`)
    if (method.type === "api") {
      const result = method.authorize ? await method.authorize(input.inputs) : undefined
      return { type: "api", ...(result === undefined ? {} : { result: cloneWireValue(result, "result") }) }
    }

    const result = await method.authorize(input.inputs)
    const flowID = handleID(instance, "flow")
    instance.flows.set(flowID, {
      plugin: registration.plugin,
      method: result.method,
      callback: result.callback,
    } as OAuthFlow)
    return {
      type: "oauth",
      flowID,
      url: result.url,
      instructions: result.instructions,
      method: result.method,
    }
  }

  async authCallback(input: { instanceID: string; flowID: string; code?: string }) {
    const instance = this.#instance(input.instanceID)
    const flow = instance.flows.get(input.flowID)
    if (!flow) throw missingHandle("auth flow", input.flowID)
    if (flow.method === "code" && input.code === undefined) {
      throw new ExtensionHostError(-32602, `Auth flow ${input.flowID} requires a code`)
    }

    try {
      const result = await (flow.method === "code"
        ? (flow.callback as (code: string) => Promise<unknown>)(input.code!)
        : (flow.callback as () => Promise<unknown>)())
      if (result && typeof result === "object" && Reflect.get(result, "type") === "success") {
        instance.flows.delete(input.flowID)
      }
      return cloneWireValue(result, "result")
    } catch (error) {
      throw pluginError(flow.plugin, "auth.callback", error)
    }
  }

  cancelAuthFlow(input: { instanceID: string; flowID: string }) {
    return { cancelled: this.#instance(input.instanceID).flows.delete(input.flowID) }
  }

  async loadAuth(input: { instanceID: string; provider: string; providerInfo: WireValue }) {
    const instance = this.#instance(input.instanceID)
    const registration = this.#auth(input.instanceID, input.provider)
    if (!registration.hook.loader) return { value: {} }

    try {
      const result = await registration.hook.loader(
        async () => {
          const result = await this.#rpc.request<{ auth: Auth | null }>("backend.auth.get", {
            instanceID: instance.id,
            providerID: input.provider,
          })
          if (!result.auth) throw new Error(`No auth is available for ${input.provider}`)
          return result.auth
        },
        cloneWireValue(input.providerInfo, "providerInfo") as never,
      )
      if (!result || typeof result !== "object" || Array.isArray(result)) {
        throw new TypeError("auth.loader must return an object")
      }

      const options = { ...result }
      const candidate = Reflect.get(options, "fetch")
      if (candidate !== undefined && typeof candidate !== "function") {
        throw new TypeError("auth.loader options.fetch must be a function")
      }
      Reflect.deleteProperty(options, "fetch")
      const plain = cloneWireValue(options, "options")
      if (!candidate) return { value: plain }

      const fetchID = handleID(instance, "fetch")
      instance.fetches.set(fetchID, {
        plugin: registration.plugin,
        provider: input.provider,
        fetch: candidate as typeof fetch,
      })
      return { value: plain, fetchID }
    } catch (error) {
      throw pluginError(registration.plugin, "auth.loader", error)
    }
  }

  async authFetch(input: {
    instanceID: string
    fetchID: string
    requestID: string
    request: {
      url: string
      method?: string
      headers?: Array<[string, string]> | Record<string, string>
      body?: StreamDescriptor
    }
  }) {
    const instance = this.#instance(input.instanceID)
    const registration = instance.fetches.get(input.fetchID)
    if (!registration) throw missingHandle("auth fetch", input.fetchID)
    if (instance.activeFetches.has(input.requestID)) {
      throw new ExtensionHostError(-32002, `Auth fetch ${input.requestID} already exists`)
    }

    const controller = new AbortController()
    const body = input.request.body ? this.#streams.remote("backend", instance.id, input.request.body) : undefined
    instance.activeFetches.set(input.requestID, { controller, body, descriptor: input.request.body })
    let returnedResponse = false
    try {
      const init = {
        method: input.request.method,
        headers: input.request.headers,
        body,
        signal: controller.signal,
        ...(body ? { duplex: "half" as const } : {}),
      } as RequestInit & { duplex?: "half" }
      const response = await registration.fetch(input.request.url, init)
      if (!(response instanceof Response)) throw new TypeError("auth loader fetch did not return a Response")
      returnedResponse = true
      return {
        status: response.status,
        statusText: response.statusText,
        headers: Array.from(response.headers.entries()),
        body: response.body
          ? this.#streams.register(instance.id, response.body, contentLength(response.headers))
          : undefined,
      }
    } catch (error) {
      throw pluginError(registration.plugin, "auth.fetch", error)
    } finally {
      instance.activeFetches.delete(input.requestID)
      if (body && !body.locked) await body.cancel("Auth fetch completed").catch(() => {})
      if (input.request.body && !returnedResponse) {
        await this.#streams.cancelRemote?.(instance.id, input.request.body, "Auth fetch failed").catch(() => {})
      }
    }
  }

  cancelAuthFetch(input: { instanceID: string; requestID: string; reason?: string }) {
    const active = this.#instance(input.instanceID).activeFetches.get(input.requestID)
    if (!active) return { cancelled: false }
    active.controller.abort(input.reason)
    void active.body?.cancel(input.reason).catch(() => {})
    if (active.descriptor) {
      void this.#streams.cancelRemote?.(input.instanceID, active.descriptor, input.reason).catch(() => {})
    }
    return { cancelled: true }
  }

  releaseAuthFetch(input: { instanceID: string; fetchID: string }) {
    return { released: this.#instance(input.instanceID).fetches.delete(input.fetchID) }
  }

  async providerModels(input: { instanceID: string; providerID: string; provider: WireValue; auth?: WireValue }) {
    const instance = this.#instance(input.instanceID)
    const registration = instance.providers.get(input.providerID)
    if (!registration?.hook.models) throw missingHandle("provider models", input.providerID)
    try {
      const result = await registration.hook.models(cloneWireValue(input.provider, "provider") as Provider, {
        auth: input.auth === undefined ? undefined : (cloneWireValue(input.auth, "auth") as Auth),
      })
      return { models: cloneWireValue(result, "models") }
    } catch (error) {
      throw pluginError(registration.plugin, "provider.models", error)
    }
  }

  async workspaceConfigure(input: { instanceID: string; registrationID: string; config: WireValue }) {
    const registration = this.#workspace(input.instanceID, input.registrationID)
    return {
      config: cloneWireValue(
        await registration.adapter.configure(cloneWireValue(input.config, "config") as WorkspaceInfo),
        "config",
      ),
    }
  }

  async workspaceCreate(input: {
    instanceID: string
    registrationID: string
    config: WireValue
    env: Record<string, string | null>
    from?: WireValue
  }) {
    const registration = this.#workspace(input.instanceID, input.registrationID)
    await registration.adapter.create(
      cloneWireValue(input.config, "config") as WorkspaceInfo,
      Object.fromEntries(Object.entries(input.env).map(([key, value]) => [key, value ?? undefined])),
      input.from === undefined ? undefined : (cloneWireValue(input.from, "from") as WorkspaceInfo),
    )
    return {}
  }

  async workspaceRemove(input: { instanceID: string; registrationID: string; config: WireValue }) {
    const registration = this.#workspace(input.instanceID, input.registrationID)
    await registration.adapter.remove(cloneWireValue(input.config, "config") as WorkspaceInfo)
    return {}
  }

  async workspaceTarget(input: { instanceID: string; registrationID: string; config: WireValue }) {
    const registration = this.#workspace(input.instanceID, input.registrationID)
    return {
      target: normalizeWorkspaceTarget(
        await registration.adapter.target(cloneWireValue(input.config, "config") as WorkspaceInfo),
      ),
    }
  }

  #assertAccepting() {
    if (this.#status === "running") return
    throw new ExtensionHostError(-32004, "Extension host is shutting down", { kind: "host_shutting_down" })
  }

  #assertOpening(instance: Instance) {
    if (this.#status === "running" && instance.status === "opening") return
    throw new ExtensionHostError(-32004, `Instance ${instance.id} is closing`, {
      kind: "instance_closing",
      instanceID: instance.id,
    })
  }

  #beginClose(instance: Instance) {
    if (instance.closePromise) return instance.closePromise
    instance.status = "closing"
    instance.closePromise = this.#disposeInstance(instance)
    return instance.closePromise
  }

  async #disposeInstance(instance: Instance) {
    for (const controller of instance.activeTools.values()) controller.abort("Instance closed")
    for (const active of instance.activeFetches.values()) {
      active.controller.abort("Instance closed")
      void active.body?.cancel("Instance closed").catch(() => {})
      if (active.descriptor) {
        void this.#streams.cancelRemote?.(instance.id, active.descriptor, "Instance closed").catch(() => {})
      }
    }
    instance.activeTools.clear()
    instance.activeFetches.clear()
    try {
      await this.#streams.cancelAll(instance.id)
    } catch (error) {
      await publishDiagnostic(this.#rpc, {
        level: "error",
        message: `Instance ${instance.id} stream cleanup failed`,
        instanceID: instance.id,
        operation: "dispose",
        error: errorData(error),
      }).catch(() => {})
    } finally {
      await instance.gateway.close().catch(() => {})
      await instance.openDone
      instance.flows.clear()
      instance.fetches.clear()

      for (const retained of instance.hooks) {
        if (instance.disposed.has(retained)) continue
        instance.disposed.add(retained)
        if (!retained.hooks.dispose) continue
        try {
          await Promise.resolve(retained.hooks.dispose())
        } catch (error) {
          await publishDiagnostic(this.#rpc, {
            level: "error",
            message: `Plugin ${retained.plugin.spec} dispose hook failed`,
            instanceID: instance.id,
            plugin: retained.plugin,
            operation: "dispose",
            error: errorData(error),
          }).catch(() => {})
        }
      }

      if (this.#instances.get(instance.id) === instance) this.#instances.delete(instance.id)
      if (this.#directories.get(instance.canonicalDirectory) === instance.id) {
        this.#directories.delete(instance.canonicalDirectory)
      }
    }
  }

  #instance(instanceID: string) {
    const instance = this.#instances.get(instanceID)
    if (!instance || instance.status !== "open") throw missingHandle("instance", instanceID)
    return instance
  }

  #auth(instanceID: string, provider: string) {
    const registration = this.#instance(instanceID).auth.get(provider)
    if (!registration) throw missingHandle("auth provider", provider)
    return registration
  }

  #workspace(instanceID: string, registrationID: string) {
    const registration = findRegistration(this.#instance(instanceID).workspaces, registrationID)
    if (!registration) throw missingHandle("workspace", registrationID)
    return registration
  }

  async #startPlugin(
    instance: Instance,
    loaded: LoadedPlugin,
    project: WireValue,
    client: ReturnType<typeof createOpencodeClient>,
    diagnostics: HostDiagnostic[],
  ) {
    for (const entrypoint of loaded.entrypoints) {
      this.#assertOpening(instance)
      const plugin: PluginMeta = {
        ...(entrypoint.id ? { id: entrypoint.id } : {}),
        spec: loaded.spec,
        entry: loaded.entry,
        index: entrypoint.index,
      }
      const workspaces = new Map<string, WorkspaceRegistration>()
      const pluginInput: PluginInput = {
        client,
        project: cloneWireValue(project, "project") as never,
        directory: instance.directory,
        worktree: instance.worktree,
        serverUrl: instance.gateway.url,
        $: this.#shell,
        experimental_workspace: {
          register: (type, adapter) => {
            workspaces.set(type, {
              registrationID: handleID(instance, "workspace"),
              plugin,
              type,
              adapter,
            })
          },
        },
      }

      try {
        const hooks = await entrypoint.server(pluginInput, loaded.options as PluginOptions | undefined)
        if (!hooks || typeof hooks !== "object" || Array.isArray(hooks)) {
          throw new TypeError("Plugin entrypoint did not return a Hooks object")
        }
        instance.hooks.push({ plugin, hooks })
        logEvent("plugin.activation.completed", {
          instance_id: instance.id,
          plugin: plugin.spec,
          plugin_id: plugin.id,
          entrypoint_index: plugin.index,
        })
        this.#assertOpening(instance)
        for (const [type, registration] of workspaces) instance.workspaces.set(type, registration)
      } catch (error) {
        if (instance.status === "closing" || this.#status !== "running") throw error
        const diagnostic = runtimeDiagnostic(plugin, "entrypoint", error)
        diagnostics.push(diagnostic)
        await publishDiagnostic(this.#rpc, toPublishedDiagnostic(instance.id, diagnostic)).catch(() => {})
      }
    }
  }

  #indexRegistrations(instance: Instance, diagnostics: HostDiagnostic[]) {
    for (const retained of instance.hooks) {
      for (const [id, definition] of Object.entries(retained.hooks.tool ?? {})) {
        try {
          instance.tools.set(id, {
            registrationID: handleID(instance, "tool"),
            plugin: retained.plugin,
            id,
            definition,
            parameters: cloneWireValue(toolParametersToJsonSchema(definition.args), `tool.${id}.parameters`),
          })
        } catch (error) {
          diagnostics.push(runtimeDiagnostic(retained.plugin, `tool:${id}`, error))
        }
      }
      if (retained.hooks.auth) {
        instance.auth.set(retained.hooks.auth.provider, { plugin: retained.plugin, hook: retained.hooks.auth })
      }
      if (retained.hooks.provider) {
        instance.providers.set(retained.hooks.provider.id, { plugin: retained.plugin, hook: retained.hooks.provider })
      }
    }
  }
}

function openResult(instance: Instance, config: WireValue, diagnostics: HostDiagnostic[]) {
  return {
    instanceID: instance.id,
    config: cloneWireValue(config, "config"),
    diagnostics: diagnostics.map(protocolDiagnostic),
    gatewayURL: instance.gateway.url.toString(),
    hooks: GENERIC_HOOKS.filter((name) =>
      instance.hooks.some((retained) => typeof retained.hooks[name] === "function"),
    ),
    tools: Array.from(instance.tools.values(), ({ registrationID, id, plugin, definition, parameters }) => ({
      registrationID,
      id,
      plugin,
      description: definition.description,
      parameters,
    })),
    auth: Array.from(instance.auth.entries(), ([provider, registration]) => authDescriptor(provider, registration)),
    providers: Array.from(instance.providers.entries(), ([provider, registration]) => ({
      provider,
      plugin: registration.plugin,
      hasModels: typeof registration.hook.models === "function",
    })),
    workspaces: Array.from(instance.workspaces.values(), ({ registrationID, type, plugin, adapter }) => ({
      registrationID,
      type,
      plugin,
      name: adapter.name,
      description: adapter.description,
    })),
  }
}

function authDescriptor(provider: string, registration: AuthRegistration) {
  return {
    provider,
    plugin: registration.plugin,
    hasLoader: typeof registration.hook.loader === "function",
    methods: registration.hook.methods.map((method, methodIndex) => ({
      type: method.type,
      label: method.label,
      methodIndex,
      hasAuthorize: typeof method.authorize === "function",
      prompts:
        method.prompts?.map((prompt, promptIndex) => ({
          type: prompt.type,
          key: prompt.key,
          message: prompt.message,
          promptIndex,
          placeholder: prompt.type === "text" ? prompt.placeholder : undefined,
          options: prompt.type === "select" ? prompt.options : undefined,
          when: prompt.when,
          hasValidate: prompt.type === "text" && typeof prompt.validate === "function",
          hasCondition: typeof prompt.condition === "function",
        })) ?? [],
    })),
  }
}

function normalizeWorkspaceTarget(target: WorkspaceTarget) {
  if (target.type === "local") return { type: "local", directory: target.directory }
  return {
    type: "remote",
    url: target.url.toString(),
    headers: target.headers ? Array.from(new Headers(target.headers).entries()) : undefined,
  }
}

function handleID(instance: Instance, type: string) {
  instance.counter += 1
  return `${instance.id}:${type}:${instance.counter}`
}

function pluginDeclarationSpec(declaration: PluginDeclaration) {
  if (typeof declaration === "string") return declaration
  if (Array.isArray(declaration)) return declaration[0]
  return declaration.spec
}

function preparationKey(input: {
  declarations: readonly PluginDeclaration[]
  defaultBaseDirectory?: string
  configurationFingerprint?: string
}) {
  const needsDefaultBaseDirectory = input.declarations.some(
    ({ spec, baseDirectory }) => !baseDirectory && (spec.startsWith(".") || spec.startsWith("file:")),
  )
  return JSON.stringify({
    configurationFingerprint: input.configurationFingerprint,
    declarations: input.declarations,
    ...(needsDefaultBaseDirectory ? { defaultBaseDirectory: input.defaultBaseDirectory } : {}),
  })
}

function findRegistration<T extends { registrationID: string }>(map: Map<string, T>, registrationID: string) {
  return Array.from(map.values()).find((registration) => registration.registrationID === registrationID)
}

function missingHandle(type: string, id: string) {
  return new ExtensionHostError(-32002, `Unknown ${type} ${id}`, { kind: "missing_handle", type, id })
}

function pluginError(plugin: PluginMeta, operation: string, error: unknown) {
  return new ExtensionHostError(-32003, `Plugin ${plugin.spec} failed during ${operation}: ${errorMessage(error)}`, {
    kind: "plugin_error",
    plugin,
    operation,
    error: errorData(error),
  })
}

function runtimeDiagnostic(plugin: PluginMeta, operation: string, error: unknown): RuntimeDiagnostic {
  return {
    level: "error",
    stage: "runtime",
    spec: plugin.spec,
    pluginID: plugin.id,
    message: `Plugin ${plugin.spec} failed during ${operation}: ${errorMessage(error)}`,
    error: errorData(error),
  }
}

function toPublishedDiagnostic(instanceID: string, diagnostic: HostDiagnostic) {
  return {
    level: diagnostic.level,
    message: diagnostic.message,
    instanceID,
    plugin: { id: "pluginID" in diagnostic ? diagnostic.pluginID : undefined, spec: diagnostic.spec },
    operation: diagnostic.stage,
    error: diagnostic.error,
  }
}

function protocolDiagnostic(diagnostic: HostDiagnostic) {
  const pluginID = "pluginID" in diagnostic ? diagnostic.pluginID : undefined
  return {
    severity: "error" as const,
    code: diagnostic.stage,
    message: diagnostic.message,
    ...(pluginID ? { plugin: pluginID } : {}),
    method: diagnostic.stage,
    data: {
      spec: diagnostic.spec,
      ...(diagnostic.error ? { error: serializableDiagnosticError(diagnostic.error) } : {}),
      ...(diagnostic.stage === "runtime" ? {} : { declarationIndex: diagnostic.declarationIndex }),
    },
  }
}

function serializableDiagnosticError(error: NonNullable<HostDiagnostic["error"]>) {
  return {
    ...(error.name ? { name: error.name } : {}),
    message: error.message,
    ...(error.stack ? { stack: error.stack } : {}),
    ...(error.cause === undefined ? {} : { cause: String(error.cause) }),
  }
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}

function contentLength(headers: Headers) {
  const value = Number(headers.get("content-length"))
  return Number.isSafeInteger(value) && value >= 0 ? value : undefined
}
