/**
 * The IDE-facing ACP server app: the default agent spine
 * ({@link @deepseek-ai/dsh-agent-spine-demo}), JSONL session persistence, and the
 * {@link @bitfun/dsh-acp/bridge} protocol bridge, owned through one ordered
 * lifecycle so ACP sessions quiesce before persistence detaches.
 *
 * Forked from `@deepseek-ai/dsh-acp-demo`
 * (deepseek-harness/packages/examples/acp-demo/src/index.ts, MIT).
 * Copyright (c) DeepSeek. See ../NOTICE.md. The only change is the transport:
 * this app mounts the IDE bridge instead of the automation-only one.
 *
 * It writes nothing to stdout, pre-creates no agents, and leaves adapters,
 * executors, and optional tools to the leaf `cordis.yml`, which must likewise
 * avoid stdout loggers. Named exports are required so Loader retains this
 * plugin's `Config` schema.
 *
 * @module @bitfun/dsh-acp
 */

import type { Context } from '@deepseek-ai/cordis'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import z from '@deepseek-ai/schemastery'
import AgentPresets from '@deepseek-ai/dsh-agent-presets'
import * as agentCore from '@deepseek-ai/dsh-agent-spine-demo'
import * as workspaceContext from '@deepseek-ai/dsh-agent-instructions'
import ToolRuntime, { type Config as ToolsConfig } from '@deepseek-ai/dsh-tools'
import JsonlSessionPersistence, {
  JsonlCompressionSchema,
  type JsonlCompression,
} from '@deepseek-ai/dsh-session-persistence-jsonl'
import * as sessionCheckpointPolicy from '@deepseek-ai/dsh-session-checkpoint-policy'
import SqliteSessionQueryEngine from '@deepseek-ai/dsh-session-query-sqlite'
import * as acp from './bridge.ts'

export const name = 'acp-ide-app'
const DEFAULT_PERSISTENCE_ROOT = './.sessions'
const DEFAULT_PRESET = 'standard'

/**
 * The shipped preset roster: `<package>/presets`, resolved from this module
 * rather than from `process.cwd()`. An ACP client launches the adapter with the
 * USER's workspace as the working directory, so a relative root would look for
 * presets inside whatever project happens to be open.
 *
 * `../presets` is correct from both `src/app.ts` and the built `lib/app.js`.
 */
const SHIPPED_PRESET_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../presets')

/**
 * App config: the swappable per-deployment values. `provider` and `model`
 * configure each agent the ACP bridge creates at `session/new`; `persona` is the
 * deployment persona (forwarded to the system-prompt plugin); `toolOrder` is the
 * explicit model-facing tool order; `tools` is the tool registry's config;
 * `persistenceRoot` is the JSONL backend's directory.
 */
export interface Config {
  /**
   * Provider route for ACP-created agents. Omitted, each session starts on the
   * deployment's default selection (`ctx.agentDefaultModel`), which is what the
   * harness's own settings document configures.
   */
  provider?: string
  /** Model name for ACP-created agents (must have a registered adapter); see {@link Config.provider}. */
  model?: string
  /** Publish model reasoning as `agent_thought_chunk` updates. Defaults to `true`. */
  reasoning?: boolean
  /** Bundled agent-loop concurrency cap; `1` is serial and omission uses its default. */
  maxParallelToolCalls?: number
  /** Deployment persona (the system-prompt plugin's `persona` config). */
  persona?: string
  /** Explicit model-facing tool order (the system-prompt plugin's `toolOrder` config). */
  toolOrder?: string[]
  /** Tool-registry config — its presentation `mode` (forwarded through agent-spine-demo). */
  tools?: ToolsConfig
  /** DeepSeek Harness home directory exposed to bash and used for local skill discovery. */
  dshHome?: string
  /** Fallback session-title limits forwarded through agent-spine-demo. */
  sessionTitle?: NonNullable<agentCore.Config['sessionTitle']>
  /** Directory for JSONL sessions and the derived query index. Defaults to `./.sessions`. */
  persistenceRoot?: string
  /**
   * Preset an ACP session composes when the client picks none. Must name a
   * directory under {@link SHIPPED_PRESET_ROOT} or the user preset root.
   * Defaults to `standard`.
   */
  defaultPreset?: string
  /**
   * Also scan `<dshHome>/.agent-presets`, so a user can drop in their own mode
   * beside the shipped three. Defaults to `true`.
   */
  userPresets?: boolean
  /** Write delta-chunk runs as packed storage rows (the JSONL backend's `packChunks`). Defaults to `true`. */
  packChunks?: boolean
  /** JSONL artifact encoding; defaults to checksummed Zstandard frames. */
  persistenceCompression?: JsonlCompression
  /** Controls automatic AGENTS.md/CLAUDE.md loading; configure a byte budget or set `false`. */
  workspaceContext: agentCore.Config['workspaceContext']
  /** Skill registry, local-provider, and model-facing consumer config forwarded to agent-spine-demo. */
  skills?: agentCore.SkillConfig
  /** Model-facing bash tool config forwarded through agent-core. */
  toolBash?: NonNullable<agentCore.Config['toolBash']>
  /** Process-local background-job admission config forwarded through agent-core. */
  jobs?: NonNullable<agentCore.Config['jobs']>
  /** Generic background-job controls forwarded through agent-core; set false to omit their tools. */
  toolJobs?: NonNullable<agentCore.Config['toolJobs']>
  /** Persisted same-session goals; owner defaults enable them, or false disables the stack and tools. */
  goals?: agentCore.GoalConfig | false
}

export const Config: z<Config> = z.object({
  provider: z.string(),
  model: z.string(),
  reasoning: z.boolean().default(true),
  maxParallelToolCalls: z.number().step(1).min(1),
  persona: z.string(),
  // The array default is forced to undefined: ABSENT means "lexicographic
  // order" (the owning dsh-system-prompt schema does the same), while
  // schemastery's native [] default would read as an invalid configured list.
  toolOrder: z.array(z.string()).default(undefined as unknown as string[]),
  tools: ToolRuntime.Config,
  dshHome: z.string(),
  sessionTitle: agentCore.SessionTitleConfigSchema,
  persistenceRoot: z.string().default(DEFAULT_PERSISTENCE_ROOT),
  defaultPreset: z.string().default(DEFAULT_PRESET),
  userPresets: z.boolean().default(true),
  packChunks: z.boolean().default(true),
  persistenceCompression: JsonlCompressionSchema,
  workspaceContext: z.union([z.const(false), workspaceContext.Config]).required(),
  skills: agentCore.SkillConfigSchema,
  toolBash: agentCore.ToolBashConfigSchema,
  jobs: agentCore.JobsConfigSchema,
  toolJobs: z.union([z.const(false), agentCore.ToolJobsConfigSchema]),
  goals: z.union([z.const(false), agentCore.GoalConfigSchema]),
})

/**
 * Compose the spine with the IDE ACP transport. The agent-spine-demo bundle
 * pre-creates NO agents (its `agents` list defaults to `[]`) and carries the
 * deployment `persona`; the JSONL backend and derived query index persist under
 * `persistenceRoot`; the ACP bridge owns stdout for JSON-RPC and creates one
 * agent per `session/new` from the provider/model pair. The composite effect
 * unloads in reverse order, keeping checkpoint and persistence listeners
 * attached until ACP agents have flushed their closing events. No logger, no
 * `hmr` — stdout stays pure.
 * @param ctx - the Cordis context this app mounts into.
 * @param config - the deployment's provider, model, persona, and persistence selection.
 */
export async function apply(ctx: Context, config: Config): Promise<void> {
  const goals = config.goals ?? {}
  const persistenceRoot = config.persistenceRoot ?? DEFAULT_PERSISTENCE_ROOT
  // A client that closes this process's stdin has ended the conversation: the
  // IDE quit, or it tore the adapter down. This lives in the app rather than in
  // a bin script because `dsh --profile` boots the composition directly and
  // never runs one — both launch routes must shut down the same way. Disposing
  // the ROOT (not just this plugin) drains the whole tree in reverse, so ACP
  // sessions quiesce before persistence detaches.
  ctx.effect(() => {
    const close = (): void => {
      void ctx.root.fiber.dispose().then(() => { process.exit(0) })
    }
    process.stdin.on('end', close)
    return () => void process.stdin.off('end', close)
  }, 'acp-ide.stdin-close')
  await ctx.effect(async function* () {
    const spine = ctx.plugin(agentCore, { ...agentCore.pickSpineConfig(config), goals })
    await spine
    yield spine.dispose
    // Before the transport, because the bridge composes every agent it creates
    // from a preset and a missing roster would leave each session on the empty
    // global tool layer.
    const presets = ctx.plugin(AgentPresets, {
      default: config.defaultPreset ?? DEFAULT_PRESET,
      roots: [{ path: SHIPPED_PRESET_ROOT, trust: 'system' }],
      includeUserRoot: config.userPresets ?? true,
    })
    await presets
    yield presets.dispose
    const persistence = ctx.plugin(JsonlSessionPersistence, {
      root: persistenceRoot,
      ...config.packChunks !== undefined ? { packChunks: config.packChunks } : {},
      ...config.persistenceCompression === undefined ? {} : { compression: config.persistenceCompression },
    })
    await persistence
    yield persistence.dispose
    const checkpoint = ctx.plugin(sessionCheckpointPolicy)
    await checkpoint
    yield checkpoint.dispose
    const query = ctx.plugin(SqliteSessionQueryEngine, { path: join(persistenceRoot, 'session-query.db') })
    await query
    yield query.dispose
    const transport = ctx.plugin(acp, {
      ...config.provider === undefined ? {} : { provider: config.provider },
      ...config.model === undefined ? {} : { model: config.model },
      ...config.reasoning === undefined ? {} : { reasoning: config.reasoning },
    })
    await transport
    yield transport.dispose
  }, 'acp-ide.composition')
}
