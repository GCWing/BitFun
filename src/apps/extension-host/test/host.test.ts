import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, mkdir, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"
import type { RpcConnection, StreamBridge, StreamDescriptor } from "../src/backend"
import { ExtensionHost } from "../src/host"
import { createGateway } from "../src/gateway"
import { preparePlugins, type LoadPluginsInput, type PreparePluginsResult } from "../src/loader"
import { HostMethodSchemas } from "../src/protocol"
import type { WireValue } from "../src/wire"

const temporaryDirectories: string[] = []
const hosts: ExtensionHost[] = []
const fixtures = path.join(import.meta.dir, "fixtures", "runtime")

afterEach(async () => {
  await Promise.all(hosts.splice(0).map((host) => host.shutdown()))
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("ExtensionHost lifecycle and hooks", () => {
  test("shares an in-flight preparation between prepare and instance open", async () => {
    const gate = Promise.withResolvers<void>()
    let prepareCalls = 0
    const harness = await createHarness(async (input) => {
      prepareCalls += 1
      await gate.promise
      return preparePlugins(input)
    })
    const directory = await projectDirectory(harness.root, "prewarm")
    const plugin = path.join(harness.root, "prewarm.ts")
    await Bun.write(plugin, 'export default { id: "fixture.prewarm", server: async () => ({}) }\n')
    const configurationFingerprint = "fixture-prewarm"
    const plugins = [{ spec: plugin }]

    const preparing = harness.host.prepare({ plugins, configurationFingerprint })
    const opening = harness.host.open({
      instanceID: "prewarm",
      project: {},
      directory,
      worktree: directory,
      config: {},
      plugins,
      configurationFingerprint,
    })
    await waitFor(() => prepareCalls === 1)
    expect(prepareCalls).toBe(1)

    gate.resolve()
    const [prepared, opened] = await Promise.all([preparing, opening])
    expect(prepared.prepared).toHaveLength(1)
    expect(opened.instanceID).toBe("prewarm")
    expect(prepareCalls).toBe(1)
  })

  test("isolates config and dispose failures while preserving shared sequential mutations", async () => {
    const harness = await createHarness()
    const directory = await projectDirectory(harness.root, "project")
    const disposeMarker = path.join(harness.root, "dispose.txt")
    const opened = await harness.host.open({
      instanceID: "lifecycle",
      project: { id: "project" },
      directory,
      worktree: directory,
      config: { order: [] },
      plugins: [
        {
          spec: path.join(fixtures, "sequence-a.js"),
          options: { configFails: true, disposeFails: true, disposeMarker },
        },
        { spec: path.join(fixtures, "sequence-b.js"), options: { disposeMarker } },
      ],
    })

    expect(opened.config).toMatchObject({ order: ["a", "b"] })
    expect(opened.configContributors).toHaveLength(2)
    expect(opened.configContributions).toHaveLength(2)
    expect(opened.configContributions[0]).toMatchObject({
      plugin: expect.objectContaining({ id: "fixture.sequence-a" }),
      outcome: "failed",
      config: { order: ["a"] },
    })
    expect(opened.configContributions[1]).toMatchObject({
      plugin: expect.objectContaining({ id: "fixture.sequence-b" }),
      outcome: "applied",
      config: { order: ["a", "b"] },
    })
    expect(opened.diagnostics).toHaveLength(1)
    expect(opened.diagnostics[0]).toMatchObject({ code: "runtime", method: "runtime" })
    expect(opened.hooks).toContain("chat.message")

    const called = await harness.host.callHook({
      instanceID: "lifecycle",
      name: "chat.message",
      input: { order: [] },
      output: { order: [] },
    })
    expect(called).toEqual({
      instanceID: "lifecycle",
      generationKey: undefined,
      revision: undefined,
      hook: "chat.message",
      input: { order: ["a", "b"] },
      output: { order: ["a", "b"] },
    })

    expect(await harness.host.close({ instanceID: "lifecycle" })).toEqual({ closed: true })
    expect(await Bun.file(disposeMarker).text()).toBe("a\nb\n")
    expect(harness.rpc.notifications).toContainEqual(
      expect.objectContaining({
        method: "backend.diagnostic.publish",
        params: expect.objectContaining({ instanceID: "lifecycle" }),
      }),
    )
  })

  test("stops an operational hook at the first failure and dispatches events independently", async () => {
    const harness = await createHarness()
    const directory = await projectDirectory(harness.root, "project")
    const hookMarker = path.join(harness.root, "hook.txt")
    const eventMarker = path.join(harness.root, "event.txt")
    await harness.host.open({
      instanceID: "failures",
      project: {},
      directory,
      worktree: directory,
      config: {},
      plugins: [
        {
          spec: path.join(fixtures, "sequence-a.js"),
          options: { hookFails: true, hookMarker, eventFails: true, eventMarker },
        },
        { spec: path.join(fixtures, "sequence-b.js"), options: { hookMarker, eventMarker } },
      ],
    })

    await expect(
      harness.host.callHook({
        instanceID: "failures",
        name: "chat.message",
        input: { order: [] },
        output: { order: [] },
      }),
    ).rejects.toMatchObject({
      code: -32003,
      data: expect.objectContaining({ operation: "chat.message" }),
    })
    expect(await Bun.file(hookMarker).text()).toBe("a\n")

    expect(harness.host.emitEvent({ instanceID: "failures", event: { type: "fixture" } })).toEqual({ accepted: true })
    await waitFor(
      async () =>
        (await Bun.file(eventMarker)
          .text()
          .catch(() => "")) === "a\nb\n",
    )
    expect(harness.rpc.notifications).toContainEqual(
      expect.objectContaining({
        method: "backend.diagnostic.publish",
        params: expect.objectContaining({ instanceID: "failures" }),
      }),
    )
  })
})

describe("ExtensionHost tools", () => {
  test("projects schemas and preserves metadata, permission, results, attachments, and cancellation", async () => {
    const harness = await createHarness()
    const directory = await projectDirectory(harness.root, "project")
    harness.rpc.onRequest("backend.tool.ask", async () => {
      await Bun.sleep(5)
      return {}
    })
    const opened = await openFull(harness, "tools", directory)
    const registration = opened.tools.find((tool) => tool.id === "fixture.echo")
    expect(registration?.parameters).toMatchObject({
      type: "object",
      properties: { value: { type: "string", description: "Value to echo" } },
      required: ["value"],
    })

    const result = await harness.host.executeTool({
      instanceID: "tools",
      registrationID: registration!.registrationID,
      executionID: "execute-1",
      args: { value: "hello" },
      context: { sessionID: "session", messageID: "message", agent: "agent", callID: "call" },
    })
    expect(result).toEqual({
      instanceID: "tools",
      generationKey: undefined,
      revision: undefined,
      executionID: "execute-1",
      result: {
        title: "echo:hello",
        output: `hello:${directory}:${directory}`,
        metadata: { sessionID: "session", callID: "call" },
        attachments: [{ type: "file", mime: "text/plain", url: "data:text/plain,fixture", filename: "fixture.txt" }],
      },
    })
    expect(harness.rpc.notifications).toContainEqual({
      method: "backend.tool.metadata",
      params: {
        instanceID: "tools",
        executionID: "execute-1",
        title: "metadata:hello",
        metadata: { phase: "before-ask" },
      },
    })
    expect(harness.rpc.requests).toContainEqual(
      expect.objectContaining({
        method: "backend.tool.ask",
        params: expect.objectContaining({
          instanceID: "tools",
          executionID: "execute-1",
          permission: "fixture.execute",
          patterns: ["hello"],
        }),
      }),
    )

    const pending = harness.host.executeTool({
      instanceID: "tools",
      registrationID: registration!.registrationID,
      executionID: "execute-2",
      args: { value: "wait", waitForAbort: true },
      context: { sessionID: "session", messageID: "message", agent: "agent" },
    })
    await waitFor(() => harness.rpc.requests.some((request) => request.params.executionID === "execute-2"))
    expect(harness.host.cancelTool({ instanceID: "tools", executionID: "execute-2" })).toEqual({ cancelled: true })
    await expect(pending).rejects.toMatchObject({
      code: -32003,
      data: expect.objectContaining({ operation: "tool:fixture.echo" }),
    })
  })

  test("keeps legacy function registration metadata JSON-compatible", async () => {
    const harness = await createHarness()
    const directory = await projectDirectory(harness.root, "legacy-tool")
    const plugin = path.join(harness.root, "legacy-tool.ts")
    await Bun.write(
      plugin,
      `export default async () => ({
        tool: {
          legacy: { description: "legacy", args: {}, execute: async () => "ok" },
        },
      })\n`,
    )
    const opened = await harness.host.open({
      instanceID: "legacy-tool",
      project: {},
      directory,
      worktree: directory,
      config: {},
      plugins: [{ spec: plugin }],
    })

    expect(opened.tools[0]!.plugin).not.toHaveProperty("id")
    expect(HostMethodSchemas["host.instance.open"].result.safeParse(opened).success).toBe(true)
  })
})

describe("ExtensionHost auth", () => {
  test("keeps auth getters live and supports prompts, API and OAuth authorization, and streaming fetch handles", async () => {
    const harness = await createHarness()
    const directory = await projectDirectory(harness.root, "project")
    let authRead = 0
    harness.rpc.onRequest("backend.auth.get", () => ({ auth: { type: "api", key: `key-${++authRead}` } }))
    const opened = await openFull(harness, "auth", directory)
    expect(opened.auth[0]).toMatchObject({
      provider: "fixture-auth",
      hasLoader: true,
      methods: [
        { type: "api", methodIndex: 0, hasAuthorize: true },
        { type: "oauth", methodIndex: 1, hasAuthorize: true },
      ],
    })

    expect(
      harness.host.evaluateAuthPrompt({
        instanceID: "auth",
        provider: "fixture-auth",
        methodIndex: 0,
        promptIndex: 0,
        operation: "validate",
        value: "bad",
        inputs: {},
      }),
    ).toEqual({ operation: "validate", error: "Token must start with ok-" })
    expect(
      harness.host.evaluateAuthPrompt({
        instanceID: "auth",
        provider: "fixture-auth",
        methodIndex: 0,
        promptIndex: 0,
        operation: "condition",
        inputs: { enabled: "yes" },
      }),
    ).toEqual({ operation: "condition", active: true })

    const first = await harness.host.loadAuth({
      instanceID: "auth",
      provider: "fixture-auth",
      providerInfo: { id: "fixture-auth" },
    })
    const second = await harness.host.loadAuth({
      instanceID: "auth",
      provider: "fixture-auth",
      providerInfo: { id: "fixture-auth" },
    })
    expect(first).toMatchObject({ value: { credential: "key-1", providerID: "fixture-auth" } })
    expect(second).toMatchObject({ value: { credential: "key-2", providerID: "fixture-auth" } })
    expect(first.fetchID).not.toBe(second.fetchID)

    expect(
      await harness.host.authorize({
        instanceID: "auth",
        provider: "fixture-auth",
        methodIndex: 0,
        inputs: { token: "ok-secret" },
      }),
    ).toEqual({
      type: "api",
      result: { type: "success", key: "ok-secret", provider: "fixture-auth", metadata: { source: "fixture" } },
    })
    const oauth = await harness.host.authorize({
      instanceID: "auth",
      provider: "fixture-auth",
      methodIndex: 1,
    })
    if (oauth.type !== "oauth") throw new Error("Expected an OAuth flow")
    if (!oauth.flowID) throw new Error("Expected an OAuth flow handle")
    const flowID = oauth.flowID
    expect(oauth).toMatchObject({ type: "oauth", method: "code", url: "https://auth.example/authorize" })
    expect(await harness.host.authCallback({ instanceID: "auth", flowID, code: "good" })).toEqual({
      type: "success",
      key: "oauth-key",
      provider: "fixture-auth",
      metadata: { code: "good" },
    })
    expect(harness.host.cancelAuthFlow({ instanceID: "auth", flowID })).toEqual({ cancelled: false })

    const requestBody = harness.streams.remoteDescriptor("auth", "request-body")
    const fetched = await harness.host.authFetch({
      instanceID: "auth",
      fetchID: first.fetchID!,
      requestID: "fetch-request",
      request: {
        url: "https://api.example/resource",
        method: "POST",
        body: requestBody,
      },
    })
    expect(fetched).toMatchObject({ status: 201, headers: expect.arrayContaining([["x-fixture-fetch", "yes"]]) })
    expect(await harness.streams.text(fetched.body!)).toBe("POST:https://api.example/resource:request-body")

    const pending = harness.host.authFetch({
      instanceID: "auth",
      fetchID: first.fetchID!,
      requestID: "fetch-cancel",
      request: { url: "https://api.example/wait" },
    })
    expect(harness.host.cancelAuthFetch({ instanceID: "auth", requestID: "fetch-cancel", reason: "stop" })).toEqual({
      cancelled: true,
    })
    await expect(pending).rejects.toMatchObject({ code: -32003 })
    expect(harness.host.cancelAuthFetch({ instanceID: "auth", requestID: "fetch-cancel" })).toEqual({
      cancelled: false,
    })
    expect(harness.host.releaseAuthFetch({ instanceID: "auth", fetchID: first.fetchID! })).toEqual({ released: true })
  })
})

describe("ExtensionHost providers and workspaces", () => {
  test("dispatches provider models and normalizes workspace operations at the wire boundary", async () => {
    const harness = await createHarness()
    const directory = await projectDirectory(harness.root, "project")
    const workspaceMarker = path.join(harness.root, "workspace.txt")
    const opened = await openFull(harness, "adapters", directory, { workspaceMarker })
    expect(opened.providers).toContainEqual(expect.objectContaining({ provider: "fixture-provider", hasModels: true }))
    expect(
      await harness.host.providerModels({
        instanceID: "adapters",
        providerID: "fixture-provider",
        provider: { id: "fixture-provider" },
        auth: { type: "api", key: "secret" },
      }),
    ).toEqual({
      models: {
        "fixture-model": { id: "fixture-model", providerID: "fixture-provider", name: "Fixture api" },
      },
    })

    const registration = opened.workspaces.find((workspace) => workspace.type === "fixture-remote")!
    const config = workspaceConfig("workspace")
    expect(
      await harness.host.workspaceConfigure({
        instanceID: "adapters",
        registrationID: registration.registrationID,
        config,
      }),
    ).toEqual({ config: { ...config, name: "configured:workspace" } })
    await harness.host.workspaceCreate({
      instanceID: "adapters",
      registrationID: registration.registrationID,
      config,
      env: { FIXTURE: "present", OMITTED: null },
      from: { ...config, id: "source" },
    })
    await harness.host.workspaceRemove({
      instanceID: "adapters",
      registrationID: registration.registrationID,
      config,
    })
    expect(await Bun.file(workspaceMarker).text()).toBe("create:workspace:present:source\nremove:workspace\n")
    expect(
      await harness.host.workspaceTarget({
        instanceID: "adapters",
        registrationID: registration.registrationID,
        config,
      }),
    ).toEqual({
      target: {
        type: "remote",
        url: "https://workspace.example/workspace?branch=dev",
        headers: [
          ["authorization", "Bearer fixture"],
          ["x-workspace", "workspace"],
        ],
      },
    })
  })
})

describe("ExtensionHost instance isolation", () => {
  test("supports independent directories, process-wide module caching, reopen, and one-shot disposal", async () => {
    const harness = await createHarness()
    const firstDirectory = await projectDirectory(harness.root, "first")
    const secondDirectory = await projectDirectory(harness.root, "second")
    const marker = path.join(harness.root, "dispose.txt")
    const first = await openFull(harness, "first", firstDirectory, { disposeMarker: marker })
    const second = await openFull(harness, "second", secondDirectory, { disposeMarker: marker })
    const firstRuntime = (first.config as { runtime: { moduleToken: string; run: number } }).runtime
    const secondRuntime = (second.config as { runtime: { moduleToken: string; run: number } }).runtime
    expect(secondRuntime.moduleToken).toBe(firstRuntime.moduleToken)
    expect(secondRuntime.run).toBe(firstRuntime.run + 1)

    await expect(openFull(harness, "duplicate", firstDirectory)).rejects.toMatchObject({ code: -32002 })
    expect(await harness.host.close({ instanceID: "first" })).toEqual({ closed: true })
    expect(await harness.host.close({ instanceID: "first" })).toEqual({ closed: false })
    const remaining = await harness.host.callHook({
      instanceID: "second",
      name: "chat.message",
      input: { trace: [] },
      output: { trace: [] },
    })
    expect(remaining.output).toEqual({ trace: ["full"] })

    const reopened = await openFull(harness, "reopened", firstDirectory, { disposeMarker: marker })
    const reopenedRuntime = (reopened.config as { runtime: { moduleToken: string; run: number } }).runtime
    expect(reopenedRuntime.moduleToken).toBe(firstRuntime.moduleToken)
    expect(reopenedRuntime.run).toBe(secondRuntime.run + 1)
    expect(await Bun.file(marker).text()).toBe(`full:${firstRuntime.run}\n`)
  })

  test("reserves instance IDs and waits for opening plugins during shutdown", async () => {
    const harness = await createHarness()
    const firstDirectory = await projectDirectory(harness.root, "first-race")
    const secondDirectory = await projectDirectory(harness.root, "second-race")
    const plugin = path.join(fixtures, "opening.js")
    const started = path.join(harness.root, "started.txt")
    const disposed = path.join(harness.root, "disposed.txt")
    const cancelledBeforeReady = harness.host.open({
      instanceID: "cancel-before-ready",
      project: {},
      directory: secondDirectory,
      worktree: secondDirectory,
      config: {},
      plugins: [],
    })
    const cancelledResult = cancelledBeforeReady.then(
      () => undefined,
      (error) => error,
    )
    expect(await harness.host.close({ instanceID: "cancel-before-ready" })).toEqual({ closed: true })
    expect(await cancelledResult).toMatchObject({ code: -32004 })

    const opening = harness.host.open({
      instanceID: "opening",
      project: {},
      directory: firstDirectory,
      worktree: firstDirectory,
      config: {},
      plugins: [{ spec: plugin, options: { started, disposed } }],
    })
    await expect(
      harness.host.open({
        instanceID: "opening",
        project: {},
        directory: secondDirectory,
        worktree: secondDirectory,
        config: {},
        plugins: [],
      }),
    ).rejects.toMatchObject({ code: -32002 })
    await waitFor(() => Bun.file(started).exists())

    const shutdown = harness.host.shutdown()
    await expect(opening).rejects.toMatchObject({ code: -32004 })
    await shutdown
    expect(await Bun.file(disposed).text()).toBe("disposed")
    await expect(
      harness.host.open({
        instanceID: "after-shutdown",
        project: {},
        directory: secondDirectory,
        worktree: secondDirectory,
        config: {},
        plugins: [],
      }),
    ).rejects.toMatchObject({ code: -32004 })
  })

  test("concurrent closes join one disposer run", async () => {
    const harness = await createHarness()
    const directory = await projectDirectory(harness.root, "close-race")
    const marker = path.join(harness.root, "close-race.txt")
    await openFull(harness, "close-race", directory, { disposeMarker: marker })

    const first = harness.host.close({ instanceID: "close-race" })
    const second = harness.host.close({ instanceID: "close-race" })
    expect(await Promise.all([first, second])).toEqual([{ closed: true }, { closed: false }])
    expect((await Bun.file(marker).text()).trim().split("\n")).toHaveLength(1)
  })

  test("continues gateway, hook, and registry cleanup when stream cancellation fails", async () => {
    const harness = await createHarness()
    const directory = await projectDirectory(harness.root, "cancel-failure")
    const marker = path.join(harness.root, "cancel-failure.txt")
    await openFull(harness, "cancel-failure", directory, { disposeMarker: marker })
    harness.streams.failCancelAll = true

    expect(await harness.host.close({ instanceID: "cancel-failure" })).toEqual({ closed: true })
    expect(await Bun.file(marker).text()).toBe("full:1\n")
    expect(await harness.host.close({ instanceID: "cancel-failure" })).toEqual({ closed: false })
  })
})

class FakeRpc implements RpcConnection {
  readonly requests: Array<{ method: string; params: Record<string, unknown> }> = []
  readonly notifications: Array<{ method: string; params: unknown }> = []
  readonly #handlers = new Map<string, (params: Record<string, unknown>) => unknown | Promise<unknown>>()

  onRequest(method: string, handler: (params: Record<string, unknown>) => unknown | Promise<unknown>) {
    this.#handlers.set(method, handler)
  }

  async request<Result>(method: string, params: unknown): Promise<Result> {
    this.requests.push({ method, params: params as Record<string, unknown> })
    const handler = this.#handlers.get(method)
    return (handler ? await handler(params as Record<string, unknown>) : {}) as Result
  }

  notify(method: string, params: unknown) {
    this.notifications.push({ method, params })
  }
}

class FakeStreams implements StreamBridge {
  readonly #streams = new Map<string, ReadableStream<Uint8Array>>()
  #counter = 0
  failCancelAll = false

  register(instanceID: string, stream: ReadableStream<Uint8Array>, length?: number) {
    const descriptor = {
      streamID: `${instanceID}:stream:${++this.#counter}`,
      ...(length === undefined ? {} : { length }),
    }
    this.#streams.set(descriptor.streamID, stream)
    return descriptor
  }

  remote(_methodPrefix: "backend" | "host", _instanceID: string, descriptor: StreamDescriptor) {
    const stream = this.#streams.get(descriptor.streamID)
    if (!stream) throw new Error(`Unknown test stream ${descriptor.streamID}`)
    return stream
  }

  async cancel(_instanceID: string, descriptor: StreamDescriptor) {
    await this.#streams.get(descriptor.streamID)?.cancel()
    this.#streams.delete(descriptor.streamID)
  }

  async cancelAll(instanceID: string) {
    if (this.failCancelAll) throw new Error("cancelAll fixture failure")
    await Promise.all(
      Array.from(this.#streams)
        .filter(([streamID]) => streamID.startsWith(`${instanceID}:`))
        .map(async ([streamID, stream]) => {
          await stream.cancel().catch(() => {})
          this.#streams.delete(streamID)
        }),
    )
  }

  remoteDescriptor(instanceID: string, body: string) {
    return this.register(instanceID, new Blob([body]).stream(), body.length)
  }

  async text(descriptor: StreamDescriptor) {
    const stream = this.#streams.get(descriptor.streamID)
    if (!stream) throw new Error(`Unknown test stream ${descriptor.streamID}`)
    return new Response(stream).text()
  }
}

async function createHarness(
  prepare: (input: LoadPluginsInput) => Promise<PreparePluginsResult> = preparePlugins,
) {
  const root = await temporaryDirectory()
  const rpc = new FakeRpc()
  const streams = new FakeStreams()
  const host = new ExtensionHost({
    rpc,
    streams,
    cacheDirectory: path.join(root, "cache"),
    gatewayFactory: createGateway,
    preparePlugins: prepare,
    shell: Bun.$,
  })
  hosts.push(host)
  return { root, rpc, streams, host }
}

async function openFull(
  harness: Awaited<ReturnType<typeof createHarness>>,
  instanceID: string,
  directory: string,
  options: Record<string, WireValue> = {},
) {
  return harness.host.open({
    instanceID,
    project: { id: "project" },
    directory,
    worktree: directory,
    config: {},
    plugins: [{ spec: path.join(fixtures, "full.js"), options }],
  })
}

async function projectDirectory(root: string, name: string) {
  const directory = path.join(root, name)
  await mkdir(directory)
  return directory
}

async function temporaryDirectory() {
  const directory = await mkdtemp(path.join(tmpdir(), "opencode-extension-host-runtime-"))
  temporaryDirectories.push(directory)
  return directory
}

async function waitFor(predicate: () => boolean | Promise<boolean>) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (await predicate()) return
    await Bun.sleep(5)
  }
  throw new Error("Timed out waiting for runtime fixture")
}

function workspaceConfig(id: string) {
  return {
    id,
    type: "fixture-remote",
    name: id,
    branch: "dev",
    directory: null,
    extra: null,
    projectID: "project",
  }
}
