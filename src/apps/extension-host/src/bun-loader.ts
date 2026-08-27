import { mkdir, realpath, stat } from "node:fs/promises"
import path from "node:path"
import {
  loadPlugins,
  preparePlugins,
  type LoadPluginsInput,
  type NpmInstaller,
  type PluginCacheStatus,
} from "./loader"
import { logEvent } from "./log"

type InstallResult = { target: string; cache: Exclude<PluginCacheStatus, "validated"> }

const installs = new Map<string, Promise<InstallResult>>()

export function loadBunPlugins(input: Omit<LoadPluginsInput, "install">) {
  return loadPlugins({ ...input, install: installNpmPlugin })
}

export function prepareBunPlugins(input: Omit<LoadPluginsInput, "install">) {
  return preparePlugins({ ...input, install: installNpmPlugin })
}

export function installNpmPlugin(input: Parameters<NpmInstaller>[0]): Promise<InstallResult> {
  const directory = path.join(
    path.resolve(input.cacheDirectory),
    "plugins",
    `${packageSlug(input.packageName ?? "plugin")}-${Bun.hash(input.spec).toString(16)}`,
  )
  const pending = installs.get(directory)
  if (pending) return pending
  if (input.allowInstall === false) return reviewCachedNpmPlugin(input, directory)
  const operation = installNpmPluginAt(input, directory).finally(() => installs.delete(directory))
  installs.set(directory, operation)
  return operation
}

async function reviewCachedNpmPlugin(input: Parameters<NpmInstaller>[0], directory: string) {
  const existing = await installedPackage(directory, input.packageName)
  if (existing) {
    logEvent("plugin.prepare.cache_hit", { plugin: input.spec, target: existing }, "debug")
    return { target: existing, cache: "hit" as const }
  }
  throw new Error(`Plugin ${input.spec} is not cached; activation review requires installation approval`)
}

async function installNpmPluginAt(input: Parameters<NpmInstaller>[0], directory: string) {
  await mkdir(directory, { recursive: true })
  const manifestPath = path.join(directory, "package.json")
  if (!(await Bun.file(manifestPath).exists())) {
    await Bun.write(manifestPath, `${JSON.stringify({ private: true }, null, 2)}\n`)
  }
  const existing = await installedPackage(directory, input.packageName)
  if (existing) {
    logEvent("plugin.prepare.cache_hit", { plugin: input.spec, target: existing }, "debug")
    return { target: existing, cache: "hit" as const }
  }
  const startedAt = performance.now()
  logEvent("plugin.prepare.install.begin", { plugin: input.spec, cache_directory: directory })
  const child = Bun.spawn({
    cmd: [process.execPath, ...bunAddArguments(input.spec)],
    cwd: directory,
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
  })
  const [code, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()])
  if (code !== 0) throw new Error(`Failed to install plugin ${input.spec}: ${stderr.trim() || stdout.trim() || `bun add exited with status ${code}`}`)
  const installed = await installedPackage(directory, input.packageName)
  if (installed) {
    logEvent("plugin.prepare.install.completed", {
      plugin: input.spec,
      target: installed,
      cache_directory: directory,
      duration_ms: Math.round(performance.now() - startedAt),
    })
    return { target: installed, cache: "installed" as const }
  }
  throw new Error(`Plugin ${input.spec} was installed but its package directory could not be found`)
}

export function bunAddArguments(spec: string) {
  return ["add", "--ignore-scripts", "--exact", "--", spec]
}

async function installedPackage(directory: string, preferred?: string) {
  const manifestPath = path.join(directory, "package.json")
  if (!(await Bun.file(manifestPath).exists())) return
  const manifest = (await Bun.file(manifestPath).json()) as Record<string, unknown>
  const dependencies = isRecord(manifest.dependencies) ? Object.keys(manifest.dependencies) : []
  const name = preferred && dependencies.includes(preferred) ? preferred : dependencies.length === 1 ? dependencies[0] : undefined
  if (!name) return
  const target = path.join(directory, "node_modules", name)
  const metadata = await stat(target).catch(() => undefined)
  return metadata?.isDirectory() ? realpath(target) : undefined
}

function packageSlug(name: string) {
  const slug = name.replaceAll(/[^A-Za-z0-9._-]/g, "-").replaceAll(/^-+|-+$/g, "")
  return slug || "plugin"
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
