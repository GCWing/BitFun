#!/usr/bin/env node
/**
 * Populate this package's node_modules by symlinking a local deepseek-harness
 * checkout.
 *
 * This is the OPTIONAL developer path. The ordinary build installs the pinned
 * `0.1.0-rc.6` train from this package's own `package-lock.json`; link against
 * a checkout when you are changing the harness and the bridge together, and
 * `scripts/prepare-dsh-profile.mjs` will then leave the linked tree alone.
 *
 * Usage: `node scripts/link-local-dsh.mjs [--repo <path-to-deepseek-harness>]`
 * or set DSH_REPO. Falls back to sibling checkouts of this repository.
 */

import { mkdirSync, readFileSync, readdirSync, rmSync, symlinkSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parseArgs } from 'node:util'

const PACKAGE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')

/** Harness packages that must resolve from this package's own node_modules. */
const HARNESS_PACKAGES = [
  // Imported by src/.
  '@deepseek-ai/cordis',
  '@deepseek-ai/schemastery',
  '@deepseek-ai/dsh-agent',
  '@deepseek-ai/dsh-agent-instructions',
  '@deepseek-ai/dsh-agent-presets',
  '@deepseek-ai/dsh-agent-spine-demo',
  '@deepseek-ai/dsh-app-boot',
  '@deepseek-ai/dsh-llm',
  '@deepseek-ai/dsh-session',
  '@deepseek-ai/dsh-session-checkpoint-policy',
  '@deepseek-ai/dsh-session-persistence-jsonl',
  '@deepseek-ai/dsh-session-query-sqlite',
  '@deepseek-ai/dsh-tools',
  '@deepseek-ai/dsh-user-approval',
  // Named by cordis.yml — the HOST plane. The Loader resolves these against the
  // config directory.
  '@deepseek-ai/dsh-agent-default-model',
  '@deepseek-ai/dsh-bash-sandbox',
  '@deepseek-ai/dsh-code-runtime',
  '@deepseek-ai/dsh-code-runtime-worker-thread',
  '@deepseek-ai/dsh-compaction-basic',
  '@deepseek-ai/dsh-credentials-local',
  '@deepseek-ai/dsh-fs-observation-policy',
  '@deepseek-ai/dsh-fs-sandbox',
  '@deepseek-ai/dsh-goal',
  '@deepseek-ai/dsh-goal-round-driver',
  '@deepseek-ai/dsh-llm-deepseek',
  '@deepseek-ai/dsh-llm-pi-ai',
  '@deepseek-ai/dsh-settings-file',
  '@deepseek-ai/dsh-repeat-tool-reminder',
  '@deepseek-ai/dsh-sandbox-local',
  '@deepseek-ai/dsh-sandbox-policy',
  '@deepseek-ai/dsh-session-projection',
  '@deepseek-ai/dsh-shell-env',
  '@deepseek-ai/dsh-skill',
  '@deepseek-ai/dsh-skill-filesystem',
  '@deepseek-ai/dsh-subagent',
  '@deepseek-ai/dsh-subagent-spawn-in-process',
  '@deepseek-ai/dsh-subprocess-local',
  '@deepseek-ai/dsh-token-meter',
  '@deepseek-ai/dsh-tool-subagent-report',
  // Named by presets/*/agent.cordis.yml — the AGENT plane. `dsh-agent-presets`
  // resolves a preset row against the preset directory, which is inside this
  // package, so these must resolve from here too.
  '@deepseek-ai/dsh-agent-tool-presentation',
  '@deepseek-ai/dsh-persona',
  '@deepseek-ai/dsh-terminal',
  '@deepseek-ai/dsh-terminal-bash',
  '@deepseek-ai/dsh-tool-bash',
  '@deepseek-ai/dsh-tool-bash-persistent',
  '@deepseek-ai/dsh-tool-fs',
  '@deepseek-ai/dsh-tool-fs-search',
  '@deepseek-ai/dsh-tool-goal',
  '@deepseek-ai/dsh-tool-jobs',
  '@deepseek-ai/dsh-tool-skill',
  '@deepseek-ai/dsh-tool-str-replace-editor',
  '@deepseek-ai/dsh-tool-subagent',
  '@deepseek-ai/dsh-tool-subagent-control',
  '@deepseek-ai/dsh-tool-todo',
  // Used only by the test suite. The Loader is a runtime dependency of the app
  // (through dsh-app-boot), but the preset specs mount it themselves, so it has
  // to resolve from here as well.
  '@deepseek-ai/cordis-plugin-loader',
  '@deepseek-ai/dsh-agent-loop',
  '@deepseek-ai/dsh-agent-loop-testkit',
]

/** Third-party packages taken from the harness checkout's own install. */
const VENDOR_PACKAGES = [
  '@agentclientprotocol/sdk',
  'tsx',
  'typescript',
  'vitest',
]

/** Directory globs (one level of `*` each) that hold harness workspace members. */
const WORKSPACE_GLOBS = [
  'vendor/*',
  'packages/*/*',
  'apps/*',
  'native/landlock-run',
  'native/landlock-run/packages/*',
]

/**
 * Find the harness checkout to link against.
 * @param explicit - a `--repo` value or DSH_REPO, when supplied.
 * @returns the absolute path of a directory that looks like deepseek-harness.
 */
function resolveHarnessRepo(explicit) {
  const candidates = explicit === undefined
    ? [
        resolve(PACKAGE_ROOT, '../../../deepseek-harness'),
        resolve(PACKAGE_ROOT, '../../../../deepseek-harness'),
        resolve(PACKAGE_ROOT, '../../../../../deepseek-harness'),
      ]
    : [resolve(process.cwd(), explicit)]
  for (const candidate of candidates) {
    try {
      const manifest = JSON.parse(readFileSync(join(candidate, 'package.json'), 'utf8'))
      if (manifest.name === '@deepseek-ai/dsh-root') return candidate
    } catch {
      // Not a harness checkout; try the next candidate.
    }
  }
  throw new Error(
    `no deepseek-harness checkout found (looked at ${candidates.join(', ')}); `
    + 'pass --repo <path> or set DSH_REPO',
  )
}

/**
 * Index every workspace member of the harness checkout by package name.
 * @param repo - the harness checkout root.
 * @returns a map from package name to its absolute directory.
 */
function indexWorkspace(repo) {
  const index = new Map()
  const dirs = []
  for (const glob of WORKSPACE_GLOBS) {
    const parts = glob.split('/')
    let level = [repo]
    for (const part of parts) {
      const next = []
      for (const base of level) {
        if (part !== '*') {
          next.push(join(base, part))
          continue
        }
        let entries = []
        try {
          entries = readdirSync(base, { withFileTypes: true })
        } catch {
          continue
        }
        for (const entry of entries) {
          if (entry.isDirectory() && !entry.name.startsWith('.')) next.push(join(base, entry.name))
        }
      }
      level = next
    }
    dirs.push(...level)
  }
  for (const dir of dirs) {
    try {
      const manifest = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8'))
      if (typeof manifest.name === 'string') index.set(manifest.name, dir)
    } catch {
      // Not a package directory.
    }
  }
  return index
}

/**
 * Replace `linkPath` with a symlink to `target`, creating scope directories.
 * @param linkPath - absolute path of the link to (re)create.
 * @param target - absolute path the link points at.
 */
function relink(linkPath, target) {
  mkdirSync(dirname(linkPath), { recursive: true })
  rmSync(linkPath, { recursive: true, force: true })
  symlinkSync(target, linkPath, 'dir')
}

const { values } = parseArgs({
  args: process.argv.slice(2),
  options: { repo: { type: 'string' } },
  strict: true,
})

const repo = resolveHarnessRepo(values.repo ?? process.env.DSH_REPO)
const workspace = indexWorkspace(repo)
const nodeModules = join(PACKAGE_ROOT, 'node_modules')
const missing = []

for (const name of HARNESS_PACKAGES) {
  const dir = workspace.get(name)
  if (dir === undefined) {
    missing.push(`${name} (no workspace member with that name in ${repo})`)
    continue
  }
  relink(join(nodeModules, name), dir)
}

for (const name of VENDOR_PACKAGES) {
  const dir = join(repo, 'node_modules', name)
  try {
    statSync(dir)
  } catch {
    missing.push(`${name} (not installed at ${dir}; run pnpm install in the harness checkout)`)
    continue
  }
  relink(join(nodeModules, name), dir)
}

// The harness checkout's .bin shims are relative to their own realpath, so a
// single link gives this package working `vitest`/`tsx`/`tsc` executables.
relink(join(nodeModules, '.bin'), join(repo, 'node_modules', '.bin'))

if (missing.length > 0) {
  process.stderr.write(`link-local-dsh: ${missing.length} dependency(ies) could not be linked:\n`)
  for (const line of missing) process.stderr.write(`  - ${line}\n`)
  process.exit(1)
}

process.stdout.write(
  `link-local-dsh: linked ${HARNESS_PACKAGES.length + VENDOR_PACKAGES.length} packages from ${repo}\n`,
)
