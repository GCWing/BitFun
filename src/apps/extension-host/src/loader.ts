import { readFile, readdir, realpath, stat } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { createHash } from "node:crypto"
import type { Plugin, PluginOptions } from "@opencode-ai/plugin"
import npmPackageArg from "npm-package-arg"
import semver from "semver"

export const OPENCODE_COMPATIBILITY_VERSION = "1.17.18"

const INDEX_FILES = ["index.ts", "index.tsx", "index.js", "index.mjs", "index.cjs"]
export type PluginDeclaration = {
  spec: string
  options?: PluginOptions
  baseDirectory?: string
}

export type PluginDeclarationInput = PluginDeclaration | string | readonly [string, PluginOptions?]

export type PluginSource = "file" | "npm"
export type PluginCacheStatus = "hit" | "installed" | "validated"

export type NormalizedPluginDeclaration = {
  declarationIndex: number
  spec: string
  resolvedSpec: string
  identity: string
  optionsDigest: string
  source: PluginSource
  packageName?: string
  options?: PluginOptions
  baseDirectory: string
}

export type PluginPackage = {
  directory: string
  manifestPath: string
  manifest: Record<string, unknown>
}

export type LoadedServerEntrypoint = {
  id?: string
  server: Plugin
  index: number
}

export type LoadedPlugin = NormalizedPluginDeclaration & {
  target: string
  entry: string
  package?: PluginPackage
  module: Record<string, unknown>
  entrypoints: LoadedServerEntrypoint[]
}

export type PreparedPlugin = NormalizedPluginDeclaration & {
  target: string
  entry: string
  cache: PluginCacheStatus
  contentHash?: string
  package?: PluginPackage
}

export type LoaderDiagnosticStage = "declaration" | "resolve" | "install" | "entry" | "compatibility" | "load" | "shape"

export type LoaderError = {
  name?: string
  message: string
  stack?: string
  cause?: string | LoaderError
}

export type LoaderDiagnostic = {
  level: "error"
  declarationIndex: number
  spec: string
  stage: LoaderDiagnosticStage
  message: string
  error?: LoaderError
}

export type NpmInstaller = (input: {
  spec: string
  packageName?: string
  cacheDirectory: string
  allowInstall?: boolean
}) => Promise<string | { target: string; cache: Exclude<PluginCacheStatus, "validated"> }>

export type LoadPluginsInput = {
  declarations: readonly PluginDeclarationInput[]
  cacheDirectory: string
  defaultBaseDirectory?: string
  compatibilityVersion?: string
  install?: NpmInstaller
  readJson?: (file: string) => Promise<Record<string, unknown>>
  satisfies?: (version: string, range: string) => boolean
  allowInstall?: boolean
}

export type LoadPluginsResult = {
  loaded: LoadedPlugin[]
  diagnostics: LoaderDiagnostic[]
}

export type PreparePluginsResult = {
  reviewed: NormalizedPluginDeclaration[]
  prepared: PreparedPlugin[]
  diagnostics: LoaderDiagnostic[]
}

type PrepareCandidateResult = { prepared: PreparedPlugin } | { diagnostic: LoaderDiagnostic }
type LoadCandidateResult = { loaded: LoadedPlugin } | { diagnostic: LoaderDiagnostic }

/**
 * Resolve and import all surviving declarations concurrently. The returned
 * entrypoints are intentionally not invoked here; callers execute them in the
 * returned order to keep plugin initialization deterministic.
 */
export async function loadPlugins(input: LoadPluginsInput): Promise<LoadPluginsResult> {
  return loadPreparedPlugins(await preparePlugins(input))
}

export async function preparePlugins(input: LoadPluginsInput): Promise<PreparePluginsResult> {
  const normalized = await normalizePluginDeclarations(input.declarations, input.defaultBaseDirectory)
  const readJson = input.readJson ?? readNodeJson
  const satisfies = input.satisfies ?? ((version, range) => semver.satisfies(version, range))
  const install = input.install ?? unavailableInstaller
  const results = await Promise.all(
    normalized.declarations.map((declaration) =>
      prepareCandidate(
        declaration,
        input.cacheDirectory,
        input.compatibilityVersion ?? OPENCODE_COMPATIBILITY_VERSION,
        install,
        readJson,
        satisfies,
        input.allowInstall ?? true,
      ),
    ),
  )

  return {
    reviewed: normalized.declarations,
    prepared: results.flatMap((result) => ("prepared" in result ? [result.prepared] : [])),
    diagnostics: [
      ...normalized.diagnostics,
      ...results.flatMap((result) => ("diagnostic" in result ? [result.diagnostic] : [])),
    ].sort((a, b) => a.declarationIndex - b.declarationIndex),
  }
}

export async function loadPreparedPlugins(input: PreparePluginsResult): Promise<LoadPluginsResult> {
  const results = await Promise.all(input.prepared.map(loadPreparedCandidate))
  return {
    loaded: results.flatMap((result) => ("loaded" in result ? [result.loaded] : [])),
    diagnostics: [
      ...input.diagnostics,
      ...results.flatMap((result) => ("diagnostic" in result ? [result.diagnostic] : [])),
    ].sort((a, b) => a.declarationIndex - b.declarationIndex),
  }
}

export const loadServerPlugins = loadPlugins

export async function normalizePluginDeclarations(
  declarations: readonly PluginDeclarationInput[],
  defaultBaseDirectory = process.cwd(),
) {
  const results = await Promise.all(
    declarations.map(async (declaration, declarationIndex) => {
      try {
        return {
          declaration: await normalizeDeclaration(declaration, declarationIndex, defaultBaseDirectory),
        }
      } catch (error) {
        return {
          diagnostic: makeDiagnostic(declarationIndex, declarationSpec(declaration), "declaration", error),
        }
      }
    }),
  )
  const seen = new Set<string>()
  const deduplicated: NormalizedPluginDeclaration[] = []

  for (const result of results.toReversed()) {
    if (!result.declaration) continue
    if (seen.has(result.declaration.identity)) continue
    seen.add(result.declaration.identity)
    deduplicated.push(result.declaration)
  }

  return {
    declarations: deduplicated.toReversed(),
    diagnostics: results.flatMap((result) => (result.diagnostic ? [result.diagnostic] : [])),
  }
}

export function extractServerEntrypoints(input: {
  module: Record<string, unknown>
  spec: string
  source: PluginSource
  package?: PluginPackage
}): LoadedServerEntrypoint[] {
  const preferred = preferredServerEntrypoint(input)
  if (preferred) return [{ ...preferred, index: 0 }]

  const seen = new Set<unknown>()
  const result: LoadedServerEntrypoint[] = []

  for (const value of Object.values(input.module)) {
    if (seen.has(value)) continue
    seen.add(value)
    const server = serverFunction(value)
    if (!server) throw new TypeError(`Plugin ${input.spec} export is not a function`)
    result.push({
      ...(legacyPluginID(value) ? { id: legacyPluginID(value) } : {}),
      server,
      index: result.length,
    })
  }

  if (!result.length) throw new TypeError(`Plugin ${input.spec} module is empty`)
  return result
}

export function parseNpmPluginSpecifier(spec: string, baseDirectory = process.cwd()) {
  const parsed = npmPackageArg(spec, baseDirectory)
  const packageName = parsed.name ?? undefined
  const canonical = parsed.saveSpec ?? parsed.fetchSpec ?? parsed.raw
  const installSpec =
    parsed.type === "directory" || parsed.type === "file"
      ? pathToFileURL(String(parsed.fetchSpec)).href
      : parsed.registry && parsed.raw === parsed.name
        ? `${parsed.name}@latest`
        : spec
  return {
    packageName,
    identity: packageName ?? String(canonical),
    installSpec,
    type: parsed.type,
  }
}

async function prepareCandidate(
  declaration: NormalizedPluginDeclaration,
  cacheDirectory: string,
  compatibilityVersion: string,
  install: NpmInstaller,
  readJson: (file: string) => Promise<Record<string, unknown>>,
  satisfies: (version: string, range: string) => boolean,
  allowInstall: boolean,
): Promise<PrepareCandidateResult> {
  let target: string
  let cache: PluginCacheStatus
  try {
    if (declaration.source === "file") {
      target = await resolveFileTarget(declaration.resolvedSpec)
      cache = "validated"
    } else {
      const installed = await install({
        spec: parseNpmPluginSpecifier(declaration.spec, declaration.baseDirectory).installSpec,
        packageName: declaration.packageName,
        cacheDirectory,
        allowInstall,
      })
      target = typeof installed === "string" ? installed : installed.target
      cache = typeof installed === "string" ? "installed" : installed.cache
    }
  } catch (error) {
    return {
      diagnostic: makeDiagnostic(
        declaration.declarationIndex,
        declaration.spec,
        declaration.source === "file" ? "resolve" : "install",
        error,
      ),
    }
  }

  let pkg: PluginPackage | undefined
  let entry: string | undefined
  try {
    pkg = await readPluginPackage(target, declaration.source === "npm", readJson)
    entry = await resolveServerEntrypoint(declaration.spec, declaration.source, target, pkg)
    if (!entry) throw new Error(`Plugin ${declaration.spec} does not expose a server entrypoint`)
  } catch (error) {
    return {
      diagnostic: makeDiagnostic(declaration.declarationIndex, declaration.spec, "entry", error),
    }
  }

  if (declaration.source === "npm" && pkg) {
    try {
      checkCompatibility(declaration.spec, pkg, compatibilityVersion, satisfies)
    } catch (error) {
      return {
        diagnostic: makeDiagnostic(declaration.declarationIndex, declaration.spec, "compatibility", error),
      }
    }
  }

  const contentHash = await pluginContentHash(target, pkg)

  return {
    prepared: {
      ...declaration,
      target,
      entry,
      cache,
      contentHash,
      package: pkg,
    },
  }
}

async function pluginContentHash(target: string, pkg?: PluginPackage): Promise<string | undefined> {
  try {
    const hash = createHash("sha256")
    const targetStat = await stat(target)
    const sourceRoot = pkg?.directory ?? (targetStat.isDirectory() ? target : path.dirname(target))
    await updateLocalSourceTreeHash(hash, sourceRoot)
    if (pkg) {
      // npm packages live below an installation root's node_modules. Bind the
      // approval to both package-local and resolver-owned lockfiles so a
      // dependency graph change cannot reuse a grant for different code.
      for (const directory of pluginDigestDirectories(pkg.directory)) {
        for (const lockName of ["package-lock.json", "pnpm-lock.yaml", "yarn.lock", "bun.lockb", "bun.lock"]) {
          const lockPath = path.join(directory, lockName)
          if (await exists(lockPath)) {
            hash.update(lockName)
            hash.update(await readFile(lockPath))
          }
        }
      }
    }
    return hash.digest("hex")
  } catch {
    return undefined
  }
}

async function updateLocalSourceTreeHash(hash: ReturnType<typeof createHash>, root: string) {
  const walk = async (directory: string): Promise<void> => {
    const entries = await readdir(directory, { withFileTypes: true })
    entries.sort((left, right) => left.name.localeCompare(right.name))
    for (const entry of entries) {
      if (entry.isSymbolicLink()) continue
      if (entry.isDirectory() && [".git", "node_modules"].includes(entry.name)) continue
      const absolute = path.join(directory, entry.name)
      if (entry.isDirectory()) {
        await walk(absolute)
        continue
      }
      if (!entry.isFile()) continue
      if (!localExecutableSourceFile(entry.name)) continue
      hash.update(path.relative(root, absolute).replaceAll(path.sep, "/"))
      hash.update("\0")
      hash.update(await readFile(absolute))
      hash.update("\0")
    }
  }
  await walk(root)
}

function localExecutableSourceFile(name: string) {
  return [".cjs", ".js", ".json", ".jsx", ".mjs", ".node", ".ts", ".tsx", ".wasm"].includes(
    path.extname(name).toLowerCase(),
  )
}

function pluginDigestDirectories(packageDirectory: string) {
  const directories = [packageDirectory]
  let current = path.dirname(packageDirectory)
  while (current !== path.dirname(current)) {
    if (path.basename(current).toLowerCase() === "node_modules") {
      directories.push(path.dirname(current))
      break
    }
    current = path.dirname(current)
  }
  return directories
}

async function loadPreparedCandidate(plugin: PreparedPlugin): Promise<LoadCandidateResult> {
  let module: Record<string, unknown>
  try {
    const cacheKey = plugin.contentHash ?? `${plugin.spec}:${plugin.declarationIndex}`
    const imported = await import(`${plugin.entry}?bitfunGeneration=${encodeURIComponent(cacheKey)}`)
    if (!isRecord(imported)) throw new Error(`Plugin ${plugin.spec} module is empty`)
    module = imported
  } catch (error) {
    return {
      diagnostic: makeDiagnostic(plugin.declarationIndex, plugin.spec, "load", error),
    }
  }

  try {
    return {
      loaded: {
        ...plugin,
        module,
        entrypoints: extractServerEntrypoints({
          module,
          spec: plugin.spec,
          source: plugin.source,
          package: plugin.package,
        }),
      },
    }
  } catch (error) {
    return {
      diagnostic: makeDiagnostic(plugin.declarationIndex, plugin.spec, "shape", error),
    }
  }
}

async function normalizeDeclaration(
  input: PluginDeclarationInput,
  declarationIndex: number,
  defaultBaseDirectory: string,
): Promise<NormalizedPluginDeclaration> {
  const declaration = declarationObject(input)
  if (typeof declaration.spec !== "string" || !declaration.spec.trim()) {
    throw new TypeError("Plugin declaration spec must be a non-empty string")
  }
  if (declaration.options !== undefined && !isRecord(declaration.options)) {
    throw new TypeError("Plugin declaration options must be an object")
  }
  if (declaration.baseDirectory !== undefined && typeof declaration.baseDirectory !== "string") {
    throw new TypeError("Plugin declaration baseDirectory must be a string")
  }

  const spec = declaration.spec.trim()
  const baseDirectory = path.resolve(declaration.baseDirectory ?? defaultBaseDirectory)
  const optionsDigest = stableJsonDigest(declaration.options ?? null)
  const source = pluginSource(spec)
  if (source === "npm") {
    const parsed = parseNpmPluginSpecifier(spec, baseDirectory)
    return {
      declarationIndex,
      spec,
      resolvedSpec: spec,
      identity: `npm:${parsed.identity}`,
      optionsDigest,
      source,
      packageName: parsed.packageName,
      options: declaration.options,
      baseDirectory,
    }
  }

  const file = spec.startsWith("file://")
    ? fileURLToPath(spec)
    : path.isAbsolute(spec) || isWindowsAbsolutePath(spec)
      ? spec
      : path.resolve(baseDirectory, spec)
  const canonical = await realpath(file).catch(() => path.resolve(file))
  const resolvedSpec = pathToFileURL(canonical).href
  return {
    declarationIndex,
    spec,
    resolvedSpec,
    identity: `file:${resolvedSpec}`,
    optionsDigest,
    source,
    options: declaration.options,
    baseDirectory,
  }
}

function stableJsonDigest(value: unknown) {
  const canonical = JSON.stringify(value, (_key, nested: unknown) => {
    if (!isRecord(nested)) return nested
    return Object.fromEntries(Object.entries(nested).sort(([left], [right]) => left.localeCompare(right)))
  })
  return createHash("sha256").update(canonical).digest("hex")
}

function declarationObject(input: PluginDeclarationInput): PluginDeclaration {
  if (typeof input === "string") return { spec: input }
  if (Array.isArray(input)) return { spec: input[0], options: input[1] }
  if (isRecord(input)) return input as PluginDeclaration
  throw new TypeError("Plugin declaration must be a string, tuple, or object")
}

function declarationSpec(input: PluginDeclarationInput) {
  if (typeof input === "string") return input
  if (Array.isArray(input)) return typeof input[0] === "string" ? input[0] : "<invalid>"
  if (isRecord(input) && typeof input.spec === "string") return input.spec
  return "<invalid>"
}

function pluginSource(spec: string): PluginSource {
  if (spec.startsWith("file://") || spec.startsWith(".") || path.isAbsolute(spec) || isWindowsAbsolutePath(spec)) {
    return "file"
  }
  return "npm"
}

async function resolveFileTarget(spec: string) {
  const file = fileURLToPath(spec)
  const info = await stat(file)
  if (!info.isDirectory()) return realpath(file)
  if (await exists(path.join(file, "package.json"))) return realpath(file)

  const index = await resolveDirectoryIndex(file)
  if (index) return index
  throw new Error(`Plugin directory ${file} is missing package.json or index file`)
}

async function readPluginPackage(
  target: string,
  required: boolean,
  readJson: (file: string) => Promise<Record<string, unknown>>,
): Promise<PluginPackage | undefined> {
  const info = await stat(target)
  const directory = info.isDirectory() ? target : path.dirname(target)
  const manifestPath = path.join(directory, "package.json")
  if (!(await exists(manifestPath))) {
    if (required) throw new Error(`Plugin package ${directory} is missing package.json`)
    return
  }

  return {
    directory: await realpath(directory),
    manifestPath,
    manifest: await readJson(manifestPath),
  }
}

async function resolveServerEntrypoint(spec: string, source: PluginSource, target: string, pkg?: PluginPackage) {
  if (pkg) {
    const exports = pkg.manifest.exports
    if (isRecord(exports)) {
      const server = extractExportValue(exports["./server"])
      if (server) return resolvePackageEntry(spec, server, "server", pkg)
    }

    const main = typeof pkg.manifest.main === "string" ? pkg.manifest.main.trim() : ""
    if (main) return resolvePackageEntry(spec, main, "main", pkg)
  }

  const info = await stat(target)
  if (!info.isDirectory()) return pathToFileURL(await realpath(target)).href
  if (source === "npm") return

  const index = await resolveDirectoryIndex(target)
  return index ? pathToFileURL(index).href : undefined
}

async function resolvePackageEntry(spec: string, raw: string, kind: string, pkg: PluginPackage) {
  const file = raw.startsWith("file://")
    ? fileURLToPath(raw)
    : path.isAbsolute(raw) || isWindowsAbsolutePath(raw)
      ? raw
      : path.resolve(pkg.directory, raw)
  if (!contains(pkg.directory, path.resolve(file))) {
    throw new Error(`Plugin ${spec} resolved ${kind} entry outside plugin directory`)
  }
  const [root, entry] = await Promise.all([realpath(pkg.directory), realpath(file)])
  if (!contains(root, entry)) throw new Error(`Plugin ${spec} resolved ${kind} entry outside plugin directory`)
  return pathToFileURL(entry).href
}

function extractExportValue(value: unknown): string | undefined {
  if (typeof value === "string") return value
  if (!isRecord(value)) return
  if (typeof value.import === "string") return value.import
  if (typeof value.default === "string") return value.default
}

async function resolveDirectoryIndex(directory: string) {
  for (const name of INDEX_FILES) {
    const file = path.join(directory, name)
    if (await exists(file)) return realpath(file)
  }
}

function checkCompatibility(
  spec: string,
  pkg: PluginPackage,
  version: string,
  satisfies: (version: string, range: string) => boolean,
) {
  const engines = pkg.manifest.engines
  if (!isRecord(engines) || typeof engines.opencode !== "string") return
  if (satisfies(version, engines.opencode)) return
  throw new Error(`Plugin ${spec} requires opencode ${engines.opencode} but running ${version}`)
}

function preferredServerEntrypoint(input: {
  module: Record<string, unknown>
  spec: string
  source: PluginSource
  package?: PluginPackage
}) {
  const value = input.module.default
  if (!isRecord(value)) return
  if (!("id" in value) && !("server" in value) && !("tui" in value)) return

  if (value.server !== undefined && typeof value.server !== "function") {
    throw new TypeError(`Plugin ${input.spec} has invalid server export`)
  }
  if (value.tui !== undefined && typeof value.tui !== "function") {
    throw new TypeError(`Plugin ${input.spec} has invalid tui export`)
  }
  if (value.server !== undefined && value.tui !== undefined) {
    throw new TypeError(`Plugin ${input.spec} must default export either server() or tui(), not both`)
  }
  if (value.server === undefined) {
    throw new TypeError(`Plugin ${input.spec} must default export an object with server()`)
  }

  const declaredID = readPluginID(value.id, input.spec)
  if (input.source === "file" && !declaredID) {
    throw new TypeError(`Path plugin ${input.spec} must export id`)
  }
  const packageID = input.source === "npm" && !declaredID ? packageName(input.package, input.spec) : undefined
  return {
    id: declaredID ?? packageID,
    server: value.server as Plugin,
  }
}

function readPluginID(value: unknown, spec: string) {
  if (value === undefined) return
  if (typeof value !== "string") throw new TypeError(`Plugin ${spec} has invalid id type ${typeof value}`)
  const id = value.trim()
  if (!id) throw new TypeError(`Plugin ${spec} has an empty id`)
  return id
}

function packageName(pkg: PluginPackage | undefined, spec: string) {
  const name = pkg?.manifest.name
  if (typeof name !== "string" || !name.trim()) {
    throw new TypeError(`Plugin package for ${spec} is missing name`)
  }
  return name.trim()
}

function serverFunction(value: unknown): Plugin | undefined {
  if (typeof value === "function") return value as Plugin
  if (!isRecord(value) || typeof value.server !== "function") return
  return value.server as Plugin
}

function legacyPluginID(value: unknown) {
  if (!isRecord(value) || typeof value.id !== "string") return
  const id = value.id.trim()
  return id || undefined
}

function packageSlug(name: string) {
  const slug = name.replaceAll(/[^A-Za-z0-9._-]/g, "-").replaceAll(/^-+|-+$/g, "")
  return slug || "plugin"
}

function contains(root: string, file: string) {
  const relative = path.relative(root, file)
  return relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative))
}

function isWindowsAbsolutePath(value: string) {
  return /^[A-Za-z]:[\\/]/.test(value)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

async function exists(file: string) {
  return stat(file)
    .then(() => true)
    .catch(() => false)
}

async function readNodeJson(file: string) {
  const value: unknown = JSON.parse(await readFile(file, "utf8"))
  if (!isRecord(value)) throw new TypeError(`${file} must contain a JSON object`)
  return value
}

function unavailableInstaller(input: Parameters<NpmInstaller>[0]): Promise<string> {
  return Promise.reject(new Error(`No installer is configured for plugin ${input.spec}`))
}

function makeDiagnostic(
  declarationIndex: number,
  spec: string,
  stage: LoaderDiagnosticStage,
  error: unknown,
): LoaderDiagnostic {
  const detail = errorInfo(error)
  return {
    level: "error",
    declarationIndex,
    spec,
    stage,
    message: detail.message,
    error: detail,
  }
}

function errorInfo(error: unknown): LoaderError {
  if (!(error instanceof Error)) return { message: String(error) }
  return {
    name: error.name,
    message: error.message,
    stack: error.stack,
    ...(error.cause === undefined
      ? {}
      : { cause: error.cause instanceof Error ? errorInfo(error.cause) : String(error.cause) }),
  }
}
