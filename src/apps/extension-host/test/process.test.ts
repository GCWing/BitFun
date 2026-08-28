import { afterEach, describe, expect, test } from "bun:test"
import { mkdir } from "node:fs/promises"
import path from "node:path"
import { RpcError } from "../src/rpc"
import { expectedHandshake, launchExtensionHost, oversizedFrameHeader, rawFrame } from "./helpers/process-host"

type Harness = Awaited<ReturnType<typeof launchExtensionHost>>

const running = new Set<Harness>()

afterEach(async () => {
  await Promise.all(Array.from(running, (harness) => harness.cleanup()))
  running.clear()
})

describe("extension host process boundary", () => {
  test("authenticates its handshake and shuts down over the control socket", async () => {
    const harness = await launch()
    expect(harness.handshake).toEqual(expectedHandshake())

    expect(await harness.peer.request<{ closed: boolean }>("host.shutdown", {})).toEqual({ closed: true })
    expect(await harness.waitForExit()).toBe(0)
    expect(await harness.stdout).toBe("")
    const stderr = await harness.stderr
    expect(stderr).toContain('"event":"startup.begin"')
    expect(stderr).toContain('"event":"rpc.send"')
    expect(stderr).toContain('"method":"backend.handshake"')
    expect(stderr).toContain('"event":"rpc.receive"')
    expect(stderr).toContain('"method":"host.shutdown"')
    expect(stderr).toContain('"event":"shutdown.requested"')
    expect(stderr).toContain('"event":"shutdown.instances_closed"')
    expect(stderr).toContain('"event":"shutdown.complete"')
    expect(stderr).not.toContain("test-rpc-token")
  })

  test("logs the plugin names activated during instance open", async () => {
    const harness = await launch()
    const directory = path.join(harness.root, "activation-project")
    const plugin = path.join(harness.root, "activation-plugin.ts")
    await mkdir(directory)
    await Bun.write(plugin, "export default async () => ({})\n")

    expect(
      await harness.peer.request<{ diagnostics: unknown[] }>("host.instance.open", {
        instanceID: "activation-instance",
        project: {},
        config: {},
        directory,
        worktree: directory,
        plugins: [{ spec: plugin }],
      }),
    ).toMatchObject({ diagnostics: [] })

    expect(await harness.peer.request<{ closed: boolean }>("host.shutdown", {})).toEqual({ closed: true })
    expect(await harness.waitForExit()).toBe(0)
    const stderr = await harness.stderr
    expect(stderr).toContain('"event":"plugin.activation.begin"')
    expect(stderr).toContain('"event":"plugin.activation.completed"')
    expect(stderr).toContain('"event":"plugin.activation.complete"')
    expect(stderr).toContain(JSON.stringify([plugin]))
  })

  test("filters debug diagnostics when the configured log level is info", async () => {
    const harness = await launchExtensionHost({ logLevel: "info" })
    running.add(harness)

    expect(await harness.peer.request<{ closed: boolean }>("host.shutdown", {})).toEqual({ closed: true })
    expect(await harness.waitForExit()).toBe(0)
    const stderr = await harness.stderr
    expect(stderr).toContain('"event":"startup.begin"')
    expect(stderr).toContain('"event":"shutdown.complete"')
    expect(stderr).not.toContain('"event":"rpc.send"')
    expect(stderr).not.toContain('"event":"rpc.receive"')
  })

  test("disables structured diagnostics when the configured log level is off", async () => {
    const harness = await launchExtensionHost({ logLevel: "off" })
    running.add(harness)

    expect(await harness.peer.request<{ closed: boolean }>("host.shutdown", {})).toEqual({ closed: true })
    expect(await harness.waitForExit()).toBe(0)
    expect(await harness.stderr).toBe("")
  })

  test("updates the structured log threshold without restarting the host", async () => {
    const harness = await launchExtensionHost({ logLevel: "debug" })
    running.add(harness)

    expect(await harness.peer.request<{ level: string }>("host.log.setLevel", { level: "off" })).toEqual({
      level: "off",
    })
    expect(await harness.peer.request<{ closed: boolean }>("host.shutdown", {})).toEqual({ closed: true })
    expect(await harness.waitForExit()).toBe(0)
    const stderr = await harness.stderr
    expect(stderr).toContain('"method":"host.log.setLevel"')
    expect(stderr).not.toContain('"method":"host.shutdown"')
    expect(stderr).not.toContain('"event":"shutdown.complete"')
  })

  test("exits when the Rust peer rejects its handshake token", async () => {
    const harness = await launchExtensionHost({ token: "wrong-token", acceptedToken: "expected-token" })
    running.add(harness)

    expect(harness.handshake).toEqual(expectedHandshake("wrong-token"))
    expect(await harness.waitForExit()).toBe(1)
    expect(await harness.stderr).toContain("Invalid extension host RPC token")
  })

  test("rejects host requests before the handshake completes", async () => {
    const gate = Promise.withResolvers<void>()
    const harness = await launchExtensionHost({ handshakeGate: gate.promise })
    running.add(harness)

    const earlyRequest = harness.peer.request("host.stream.cancel", {})
    const outcome = await Promise.race([
      earlyRequest.then(
        (value) => ({ kind: "resolved" as const, value }),
        (error) => ({ kind: "rejected" as const, error }),
      ),
      Bun.sleep(1_000).then(() => ({ kind: "timeout" as const })),
    ])
    gate.resolve()
    if (outcome.kind === "timeout") await earlyRequest.catch(() => {})
    expect(outcome).toMatchObject({ kind: "rejected", error: { code: -32601 } })
    expect(
      await requestAfterHandshakeReply(() => harness.peer.request<{ closed: boolean }>("host.shutdown", {})),
    ).toEqual({ closed: true })
    expect(await harness.waitForExit()).toBe(0)
  })

  test("returns structured invalid-parameter errors without dropping the connection", async () => {
    const harness = await launch()
    const error = (await harness.peer
      .request("host.instance.close", { wrong: true })
      .catch((value) => value)) as RpcError

    expect(error).toBeInstanceOf(RpcError)
    expect(error).toMatchObject({
      code: -32602,
      data: { kind: "invalid_params", method: "host.instance.close" },
    })
    expect(await harness.peer.request<{ closed: boolean }>("host.instance.close", { instanceID: "missing" })).toEqual({
      closed: false,
    })
    expect(
      await harness.peer.request<{ cancelled: boolean }>("host.stream.cancel", {
        instanceID: "missing",
        streamID: "missing-stream",
      }),
    ).toEqual({ cancelled: false })
    expect(await harness.peer.request<{ closed: boolean }>("host.shutdown", {})).toEqual({ closed: true })
    expect(await harness.waitForExit()).toBe(0)
  })

  test("terminates cleanly after a malformed JSON frame", async () => {
    const harness = await launch()
    await harness.peer.request("host.instance.close", { instanceID: "ready" })
    harness.write(rawFrame("{"))

    expect(await harness.waitForExit()).toBe(1)
    expect(await harness.stderr).toContain("Invalid JSON-RPC JSON payload")
  })

  test("rejects an oversized frame from the negotiated limit", async () => {
    const harness = await launchExtensionHost({ maxFrameBytes: 64 * 1024 })
    running.add(harness)
    await harness.peer.request("host.instance.close", { instanceID: "ready" })
    harness.write(oversizedFrameHeader(64 * 1024 + 1))

    expect(await harness.waitForExit()).toBe(1)
    expect(await harness.stderr).toContain("frame length 65537 exceeds limit 65536")
  })

  test("disposes open instances when the Rust-owned socket reaches EOF", async () => {
    const harness = await launch()
    const directory = path.join(harness.root, "project")
    const marker = path.join(harness.root, "disposed.txt")
    const plugin = path.join(harness.root, "dispose-plugin.ts")
    await mkdir(directory)
    await Bun.write(
      plugin,
      `export default async (_input, options) => ({
        async dispose() {
          await Bun.write(options.marker, "disposed")
        },
      })\n`,
    )
    const result = await harness.peer.request<{ diagnostics: unknown[] }>("host.instance.open", {
      instanceID: "eof-instance",
      project: {},
      config: {},
      directory,
      worktree: directory,
      plugins: [{ spec: plugin, options: { marker } }],
    })
    expect(result.diagnostics).toEqual([])

    harness.peer.close()
    expect(await harness.waitForExit()).toBe(0)
    expect(await Bun.file(marker).text()).toBe("disposed")
  })

  test("allows concurrent out-of-order opens with reentrant backend HTTP", async () => {
    const harness = await launch()
    const plugin = path.join(harness.root, "initializing-plugin.ts")
    const slowDirectory = path.join(harness.root, "slow")
    const fastDirectory = path.join(harness.root, "fast")
    await Promise.all([mkdir(slowDirectory), mkdir(fastDirectory)])
    await Bun.write(
      plugin,
      `export default async (input, options) => {
        await Bun.sleep(options.delay)
        const response = await fetch(new URL("/initialize?name=" + options.name, input.serverUrl))
        return {
          config(config) {
            config.initialized = { name: options.name, status: response.status }
          },
        }
      }\n`,
    )
    const forwarded: string[] = []
    harness.peer.handle("backend.http.request", (value) => {
      forwarded.push((value as { path: string }).path)
      return { status: 204, headers: [] }
    })

    const slow = harness.peer.request<OpenResult>("host.instance.open", {
      instanceID: "slow-instance",
      project: {},
      config: {},
      directory: slowDirectory,
      worktree: slowDirectory,
      plugins: [{ spec: plugin, options: { delay: 150, name: "slow" } }],
    })
    await Bun.sleep(10)
    const fast = harness.peer.request<OpenResult>("host.instance.open", {
      instanceID: "fast-instance",
      project: {},
      config: {},
      directory: fastDirectory,
      worktree: fastDirectory,
      plugins: [{ spec: plugin, options: { delay: 0, name: "fast" } }],
    })

    expect(await Promise.race([slow.then(() => "slow"), fast.then(() => "fast")])).toBe("fast")
    expect((await fast).config).toEqual({ initialized: { name: "fast", status: 204 } })
    expect((await slow).config).toEqual({ initialized: { name: "slow", status: 204 } })
    expect(forwarded).toEqual(["/initialize?name=fast", "/initialize?name=slow"])

    await Promise.all([
      harness.peer.request("host.instance.close", { instanceID: "slow-instance" }),
      harness.peer.request("host.instance.close", { instanceID: "fast-instance" }),
    ])
    expect(await harness.peer.request<{ closed: boolean }>("host.shutdown", {})).toEqual({ closed: true })
    expect(await harness.waitForExit()).toBe(0)
  }, 10_000)
})

async function requestAfterHandshakeReply<Result>(request: () => Promise<Result>) {
  const deadline = performance.now() + 1_000
  for (;;) {
    try {
      return await request()
    } catch (error) {
      if (!(error instanceof RpcError) || error.code !== -32601 || performance.now() >= deadline) throw error
      await Bun.sleep(5)
    }
  }
}

type OpenResult = {
  config: Record<string, unknown>
}

async function launch() {
  const harness = await launchExtensionHost()
  running.add(harness)
  return harness
}
