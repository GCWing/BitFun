import { afterEach, describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, realpath, rm, symlink } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"
import { pathToFileURL } from "node:url"
import { z } from "zod"
import {
  extractServerEntrypoints,
  loadPreparedPlugins,
  loadPlugins,
  normalizePluginDeclarations,
  parseNpmPluginSpecifier,
  preparePlugins,
} from "../src/loader"
import { bunAddArguments, installNpmPlugin } from "../src/bun-loader"
import { toolParametersToJsonSchema, validateToolArguments } from "../src/tool-schema"
import { WireValueError, cloneWireValue } from "../src/wire"

const temporaryDirectories: string[] = []
const fixtures = path.join(import.meta.dir, "fixtures", "loader")

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("plugin loader", () => {
  test("terminates bun add options before an untrusted package spec", () => {
    expect(bunAddArguments("-malicious-spec")).toEqual(["add", "--ignore-scripts", "--exact", "--", "-malicious-spec"])
  })

  test("normalizes relative specs and keeps the final declaration for each identity", async () => {
    const directory = await temporaryDirectory()
    const plugin = path.join(directory, "plugin.ts")
    await Bun.write(plugin, 'export default { id: "fixture.dedupe", server: async () => ({}) }\n')

    const result = await normalizePluginDeclarations([
      { spec: pathToFileURL(plugin).href, options: { order: 1 } },
      { spec: "./plugin.ts", options: { order: 2 }, baseDirectory: directory },
    ])

    expect(result.diagnostics).toEqual([])
    expect(result.declarations).toHaveLength(1)
    expect(result.declarations[0]?.options).toEqual({ order: 2 })
    expect(result.declarations[0]?.optionsDigest).toMatch(/^[0-9a-f]{64}$/)
    expect(result.declarations[0]?.resolvedSpec).toBe(pathToFileURL(await realpath(plugin)).href)
  })

  test("binds normalized source and declaration options into stable review facts", async () => {
    const directory = await temporaryDirectory()
    const plugin = path.join(directory, "plugin.ts")
    await Bun.write(plugin, 'export default { id: "fixture.review", server: async () => ({}) }\n')

    const first = await normalizePluginDeclarations([
      { spec: "./plugin.ts", baseDirectory: directory, options: { nested: { right: 2, left: 1 } } },
    ])
    const reordered = await normalizePluginDeclarations([
      { spec: pathToFileURL(plugin).href, baseDirectory: directory, options: { nested: { left: 1, right: 2 } } },
    ])
    const changed = await normalizePluginDeclarations([
      { spec: "./plugin.ts", baseDirectory: directory, options: { nested: { left: 1, right: 3 } } },
    ])

    expect(first.declarations[0]?.identity).toBe(reordered.declarations[0]?.identity)
    expect(first.declarations[0]?.resolvedSpec).toBe(reordered.declarations[0]?.resolvedSpec)
    expect(first.declarations[0]?.optionsDigest).toBe(reordered.declarations[0]?.optionsDigest)
    expect(first.declarations[0]?.optionsDigest).not.toBe(changed.declarations[0]?.optionsDigest)
  })

  test("prefers the default object-form plugin and exposes entrypoints without executing them", async () => {
    const result = await loadPlugins({
      declarations: [path.join(fixtures, "preferred.ts")],
      cacheDirectory: await temporaryDirectory(),
    })

    expect(result.diagnostics).toEqual([])
    expect(result.loaded).toHaveLength(1)
    expect(result.loaded[0]?.entrypoints).toHaveLength(1)
    expect(result.loaded[0]?.entrypoints[0]?.id).toBe("fixture.preferred")
    expect(result.loaded[0]?.entrypoints[0]?.index).toBe(0)
  })

  test("prepares plugin entrypoints without importing their modules", async () => {
    const directory = await temporaryDirectory()
    const marker = path.join(directory, "imported.txt")
    const plugin = path.join(directory, "plugin.ts")
    await Bun.write(
      plugin,
      `await Bun.write(${JSON.stringify(marker)}, "imported")\nexport default { id: "fixture.prepared", server: async () => ({}) }\n`,
    )

    const prepared = await preparePlugins({ declarations: [plugin], cacheDirectory: directory })

    expect(prepared.diagnostics).toEqual([])
    expect(prepared.prepared).toHaveLength(1)
    expect(await Bun.file(marker).exists()).toBe(false)

    const loaded = await loadPreparedPlugins(prepared)
    expect(loaded.loaded[0]?.entrypoints[0]?.id).toBe("fixture.prepared")
    expect(await Bun.file(marker).exists()).toBe(true)
  })

  test("changes the content digest when a local plugin dependency changes", async () => {
    const directory = await temporaryDirectory()
    const plugin = path.join(directory, "plugin.ts")
    const dependency = path.join(directory, "dependency.ts")
    await Bun.write(dependency, 'export const id = "first"\n')
    await Bun.write(
      plugin,
      'import { id } from "./dependency"\nexport default { id, server: async () => ({}) }\n',
    )

    const first = await preparePlugins({ declarations: [plugin], cacheDirectory: directory })
    await Bun.write(dependency, 'export const id = "second"\n')
    const second = await preparePlugins({ declarations: [plugin], cacheDirectory: directory })

    expect(first.prepared[0]?.contentHash).toMatch(/^[0-9a-f]{64}$/)
    expect(second.prepared[0]?.contentHash).toMatch(/^[0-9a-f]{64}$/)
    expect(second.prepared[0]?.contentHash).not.toBe(first.prepared[0]?.contentHash)
  })

  test("deduplicates legacy exports by exported value identity", async () => {
    const module = await import(pathToFileURL(path.join(fixtures, "legacy.ts")).href)
    const entrypoints = extractServerEntrypoints({
      module,
      source: "file",
      spec: path.join(fixtures, "legacy.ts"),
    })

    expect(entrypoints).toHaveLength(1)
    expect(entrypoints[0]?.index).toBe(0)
  })

  test("resolves package ./server import before default and main", async () => {
    const directory = await temporaryDirectory()
    await Bun.write(
      path.join(directory, "package.json"),
      JSON.stringify({
        name: "fixture-entry",
        exports: { "./server": { import: "./import.ts", default: "./default.ts" } },
        main: "./main.ts",
      }),
    )
    await Bun.write(
      path.join(directory, "import.ts"),
      'export default { id: "fixture.import", server: async () => ({}) }\n',
    )
    await Bun.write(path.join(directory, "default.ts"), 'throw new Error("default entry loaded")\n')
    await Bun.write(path.join(directory, "main.ts"), 'throw new Error("main entry loaded")\n')

    const result = await loadPlugins({
      declarations: [directory],
      cacheDirectory: await temporaryDirectory(),
    })

    expect(result.diagnostics).toEqual([])
    expect(result.loaded[0]?.entrypoints[0]?.id).toBe("fixture.import")
  })

  test("uses a local directory index when no package manifest exists", async () => {
    const directory = await temporaryDirectory()
    await Bun.write(
      path.join(directory, "index.ts"),
      'export default { id: "fixture.index", server: async () => ({}) }\n',
    )

    const result = await loadPlugins({
      declarations: [directory],
      cacheDirectory: await temporaryDirectory(),
    })

    expect(result.diagnostics).toEqual([])
    expect(result.loaded[0]?.entrypoints[0]?.id).toBe("fixture.index")
  })

  test("falls back to package main when no server export exists", async () => {
    const directory = await temporaryDirectory()
    await Bun.write(path.join(directory, "package.json"), JSON.stringify({ name: "fixture-main", main: "main.ts" }))
    await Bun.write(
      path.join(directory, "main.ts"),
      'export default { id: "fixture.main", server: async () => ({}) }\n',
    )

    const result = await loadPlugins({ declarations: [directory], cacheDirectory: await temporaryDirectory() })

    expect(result.diagnostics).toEqual([])
    expect(result.loaded[0]?.entrypoints[0]?.id).toBe("fixture.main")
  })

  test("isolates import and module-shape failures from successful neighbors", async () => {
    const directory = await temporaryDirectory()
    await Bun.write(path.join(directory, "bad-import.ts"), 'throw new Error("bad import")\n')
    await Bun.write(path.join(directory, "bad-shape.ts"), "export const value = 1\n")
    await Bun.write(
      path.join(directory, "good.ts"),
      'export default { id: "fixture.good", server: async () => ({}) }\n',
    )

    const result = await loadPlugins({
      declarations: [
        path.join(directory, "bad-import.ts"),
        path.join(directory, "bad-shape.ts"),
        path.join(directory, "good.ts"),
      ],
      cacheDirectory: await temporaryDirectory(),
    })

    expect(result.loaded.flatMap((plugin) => plugin.entrypoints.map((entrypoint) => entrypoint.id))).toEqual([
      "fixture.good",
    ])
    expect(result.diagnostics.map((diagnostic) => diagnostic.stage)).toEqual(["load", "shape"])
  })

  test("rejects invalid object-form path plugins and TUI/server hybrids", () => {
    expect(() =>
      extractServerEntrypoints({
        module: { default: { server: async () => ({}) } },
        source: "file",
        spec: "missing-id.ts",
      }),
    ).toThrow("must export id")
    expect(() =>
      extractServerEntrypoints({
        module: { default: { id: "fixture.hybrid", server: async () => ({}), tui: async () => ({}) } },
        source: "file",
        spec: "hybrid.ts",
      }),
    ).toThrow("either server() or tui()")
  })

  test("imports candidates concurrently but returns them in declaration order", async () => {
    const directory = await temporaryDirectory()
    const marker = path.join(directory, "imports.txt")
    await Bun.write(
      path.join(directory, "slow.ts"),
      [
        "await Bun.sleep(20)",
        `await Bun.write(${JSON.stringify(marker)}, (await Bun.file(${JSON.stringify(marker)}).text().catch(() => "")) + "slow\\n")`,
        'export default { id: "fixture.slow", server: async () => ({}) }',
      ].join("\n"),
    )
    await Bun.write(
      path.join(directory, "fast.ts"),
      [
        `await Bun.write(${JSON.stringify(marker)}, (await Bun.file(${JSON.stringify(marker)}).text().catch(() => "")) + "fast\\n")`,
        'export default { id: "fixture.fast", server: async () => ({}) }',
      ].join("\n"),
    )

    const result = await loadPlugins({
      declarations: [path.join(directory, "slow.ts"), path.join(directory, "fast.ts")],
      cacheDirectory: await temporaryDirectory(),
    })

    expect(result.loaded.flatMap((plugin) => plugin.entrypoints.map((entrypoint) => entrypoint.id))).toEqual([
      "fixture.slow",
      "fixture.fast",
    ])
    expect(await Bun.file(marker).text()).toBe("fast\nslow\n")
  })

  test("rejects a package entry that escapes through a symlink", async () => {
    const directory = await temporaryDirectory()
    const plugin = path.join(directory, "plugin")
    const outside = path.join(directory, "outside")
    await mkdir(plugin)
    await mkdir(outside)
    await Bun.write(
      path.join(plugin, "package.json"),
      JSON.stringify({ exports: { "./server": "./escape/server.ts" } }),
    )
    await Bun.write(
      path.join(outside, "server.ts"),
      'export default { id: "fixture.escape", server: async () => ({}) }\n',
    )
    await symlink(outside, path.join(plugin, "escape"), process.platform === "win32" ? "junction" : "dir")

    const result = await loadPlugins({
      declarations: [plugin],
      cacheDirectory: await temporaryDirectory(),
    })

    expect(result.loaded).toEqual([])
    expect(result.diagnostics[0]?.stage).toBe("entry")
    expect(result.diagnostics[0]?.message).toContain("outside plugin directory")
  })

  test("checks npm engines against OpenCode 1.17.18 and isolates failures", async () => {
    const directory = await temporaryDirectory()
    const incompatible = path.join(directory, "incompatible")
    const compatible = path.join(directory, "compatible")
    await Promise.all([mkdir(incompatible), mkdir(compatible)])
    await Bun.write(
      path.join(incompatible, "package.json"),
      JSON.stringify({ name: "incompatible", engines: { opencode: ">=2" }, main: "./index.ts" }),
    )
    await Bun.write(path.join(incompatible, "index.ts"), "export default { server: async () => ({}) }\n")
    await Bun.write(
      path.join(compatible, "package.json"),
      JSON.stringify({ name: "compatible", engines: { opencode: "^1.17.0" }, main: "./index.ts" }),
    )
    await Bun.write(path.join(compatible, "index.ts"), "export default { server: async () => ({}) }\n")

    const result = await loadPlugins({
      declarations: ["incompatible@1.0.0", "compatible@1.0.0"],
      cacheDirectory: await temporaryDirectory(),
      install: async (input) => (input.packageName === "incompatible" ? incompatible : compatible),
    })

    expect(result.loaded.map((plugin) => plugin.spec)).toEqual(["compatible@1.0.0"])
    expect(result.loaded[0]?.entrypoints[0]?.id).toBe("compatible")
    expect(result.diagnostics).toHaveLength(1)
    expect(result.diagnostics[0]?.stage).toBe("compatibility")
  })

  test("parses scoped, alias, tarball, and git npm specs without inventing filesystem package names", () => {
    expect(parseNpmPluginSpecifier("@scope/plugin@2.3.4")).toMatchObject({
      packageName: "@scope/plugin",
      identity: "@scope/plugin",
    })
    expect(parseNpmPluginSpecifier("alias-plugin@npm:@scope/plugin@2.3.4")).toMatchObject({
      packageName: "alias-plugin",
      identity: "alias-plugin",
    })
    expect(parseNpmPluginSpecifier("https://example.com/plugin.tgz")).toMatchObject({
      packageName: undefined,
      type: "remote",
    })
    expect(parseNpmPluginSpecifier("github:example/plugin#main")).toMatchObject({
      packageName: undefined,
      type: "git",
    })
    expect(parseNpmPluginSpecifier("file:./plugin.tgz", "/tmp/extension-host-base").installSpec).toBe(
      pathToFileURL(path.resolve("/tmp/extension-host-base/plugin.tgz")).href,
    )
  })

  // Bun 1.3.x cannot install local file dependencies reliably on Windows
  // (oven-sh/bun#13379). File plugin declarations use the direct validated
  // loader path in production; this integration case remains covered on Unix.
  test.skipIf(process.platform === "win32")("installs npm packages with lifecycle scripts disabled", async () => {
    const directory = await temporaryDirectory()
    const source = path.join(directory, "source")
    const marker = path.join(directory, "postinstall.txt")
    await mkdir(source)
    await Bun.write(
      path.join(source, "package.json"),
      JSON.stringify({
        name: "fixture-install",
        version: "1.0.0",
        main: "./index.js",
        scripts: { postinstall: `bun -e 'Bun.write(${JSON.stringify(marker)}, "ran")'` },
      }),
    )
    await Bun.write(path.join(source, "index.js"), "export default { server: async () => ({}) }\n")

    const target = await installNpmPlugin({
      spec: pathToFileURL(source).href,
      packageName: "fixture-install",
      cacheDirectory: path.join(directory, "cache"),
    })

    expect(target.cache).toBe("installed")
    expect(await Bun.file(path.join(target.target, "package.json")).exists()).toBe(true)
    expect(await Bun.file(marker).exists()).toBe(false)
  })

  test("review preparation forwards the non-installing contract to npm resolution", async () => {
    const directory = await temporaryDirectory()
    let allowInstall: boolean | undefined
    const result = await preparePlugins({
      declarations: ["fixture-review@1.0.0"],
      cacheDirectory: path.join(directory, "cache"),
      allowInstall: false,
      install: async (input) => {
        allowInstall = input.allowInstall
        throw new Error("activation review requires installation approval")
      },
    })

    expect(allowInstall).toBe(false)
    expect(result.reviewed[0]).toMatchObject({
      spec: "fixture-review@1.0.0",
      source: "npm",
      identity: "npm:fixture-review",
    })
    expect(result.reviewed[0]?.optionsDigest).toMatch(/^[0-9a-f]{64}$/)
    expect(result.prepared).toEqual([])
    expect(result.diagnostics[0]?.stage).toBe("install")
    expect(result.diagnostics[0]?.message).toContain("approval")
  })

  test("reuses an installed npm package directory without reinstalling it", async () => {
    const directory = await temporaryDirectory()
    const cacheDirectory = path.join(directory, "cache")
    const spec = "fixture-cache-hit@1.0.0"
    const installDirectory = path.join(
      cacheDirectory,
      "plugins",
      `fixture-cache-hit-${Bun.hash(spec).toString(16)}`,
    )
    const target = path.join(installDirectory, "node_modules", "fixture-cache-hit")
    await mkdir(target, { recursive: true })
    await Bun.write(
      path.join(installDirectory, "package.json"),
      JSON.stringify({ dependencies: { "fixture-cache-hit": "1.0.0" } }),
    )
    await Bun.write(path.join(target, "package.json"), JSON.stringify({ name: "fixture-cache-hit", version: "1.0.0" }))

    const resolved = await installNpmPlugin({
      spec,
      packageName: "fixture-cache-hit",
      cacheDirectory,
    })

    expect(resolved).toEqual({ target: await realpath(target), cache: "hit" })
  })
})

describe("wire values", () => {
  test("clones plain JSON values without retaining aliases", () => {
    const child = { value: 1 }
    const input = { left: child, right: child }
    const cloned = cloneWireValue(input) as { left: { value: number }; right: { value: number } }

    expect(cloned).toEqual(input)
    expect(cloned).not.toBe(input)
    expect(cloned.left).not.toBe(child)
    expect(cloned.left).not.toBe(cloned.right)
  })

  test("matches JSON omission semantics for nested undefined values", () => {
    expect(cloneWireValue({ missing: undefined, values: [undefined, 1] })).toEqual({ values: [null, 1] })
    expect(() => cloneWireValue(undefined)).toThrow("Wire value at $ cannot contain undefined")
  })

  test.each([
    [{ auth: { fetch: () => {} } }, "$.auth.fetch", "function"],
    [{ count: 1n }, "$.count", "BigInt"],
    [{ values: [Number.NaN] }, "$.values[0]", "non-finite"],
  ])("rejects unsupported values with their path", (value, location, kind) => {
    expect(() => cloneWireValue(value)).toThrow(location)
    expect(() => cloneWireValue(value)).toThrow(kind)
  })

  test("reports the source path of a cycle", () => {
    const value: { child?: unknown } = {}
    value.child = value

    expect(() => cloneWireValue(value)).toThrow("$.child contains a cycle referencing $")
    expect(() => cloneWireValue(value)).toThrow(WireValueError)
  })
})

describe("plugin tool schemas", () => {
  test("converts Zod argument maps and preserves metadata", () => {
    const schema = toolParametersToJsonSchema({
      query: z.string().describe("Search query"),
      limit: z.number().int().optional(),
    }) as Record<string, unknown>

    expect(schema.type).toBe("object")
    expect(schema.properties).toEqual({
      query: { type: "string", description: "Search query" },
      limit: { type: "integer", minimum: -9007199254740991, maximum: 9007199254740991 },
    })
    expect(schema.required).toEqual(["query"])
  })

  test("projects legacy definitions and only requires valid schema entries", () => {
    expect(
      toolParametersToJsonSchema({
        query: { type: "string" },
        enabled: true,
        ignored: "not-json-schema",
      }),
    ).toEqual({
      type: "object",
      properties: { query: { type: "string" }, enabled: true },
      required: ["query", "enabled"],
    })
  })

  test("validates Zod argument maps and passes legacy values through", () => {
    expect(validateToolArguments({ count: z.number().int() }, { count: 2, ignored: true })).toEqual({ count: 2 })
    expect(() => validateToolArguments({ count: z.number().int() }, { count: 2.5 })).toThrow()

    const legacy = { count: 2.5 }
    expect(validateToolArguments({ count: { type: "integer" } }, legacy)).toBe(legacy)
  })
})

async function temporaryDirectory() {
  const directory = await mkdtemp(path.join(tmpdir(), "opencode-extension-host-"))
  temporaryDirectories.push(directory)
  return directory
}
