/**
 * In-memory ACP transport fixture over the real agent factory and loop.
 *
 * Adapted from `deepseek-harness/packages/acp/acp/tests/harness.ts` (MIT,
 * copyright (c) DeepSeek); see ../NOTICE.md. The only substantive change is the
 * plugin under test — `../src/bridge.ts` instead of `@deepseek-ai/dsh-acp` —
 * plus `orderedEvents`, which records session updates and permission requests
 * on one timeline so a test can assert their relative order.
 */

import { dirname, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { Context } from '@deepseek-ai/cordis'
import Loader from '@deepseek-ai/cordis-plugin-loader'
import AgentPresets from '@deepseek-ai/dsh-agent-presets'
import {
  ClientSideConnection,
  ndJsonStream,
  type Agent as AcpAgent,
  type Client,
  type RequestPermissionRequest,
  type RequestPermissionResponse,
  type SessionConfigOption,
  type SessionNotification,
  type Stream,
} from '@agentclientprotocol/sdk'
import { type GenerateOptions, LlmAdapter, type StreamChunk } from '@deepseek-ai/dsh-llm'
import AgentLoop from '@deepseek-ai/dsh-agent-loop'
import { mountAgentLoopTestDependencies } from '@deepseek-ai/dsh-agent-loop-testkit'
import JsonlSessionPersistence from '@deepseek-ai/dsh-session-persistence-jsonl'
import * as AcpPlugin from '../src/bridge.ts'
import type { AcpConfig } from '../src/bridge.ts'

/** One provider route the fixture serves, and the models it advertises. */
export interface CatalogProvider {
  /** Human-readable provider name — what a grouped model picker labels the group with. */
  name: string
  /** Advertised models, in the order a picker lists them. */
  models: { id: string; name: string; description?: string }[]
}

/**
 * The single-provider, single-model catalog every test gets unless it asks for
 * more, which is what the fixture advertised before it could advertise
 * anything else.
 */
const DEFAULT_CATALOG: Record<string, CatalogProvider> = {
  mock: { name: 'Mock', models: [{ id: 'mock', name: 'Mock' }] },
}

/** Scripted adapter for protocol tests. */
class MockAdapter extends LlmAdapter {
  readonly requests: GenerateOptions[] = []
  private readonly script: (StreamChunk[] | 'hang')[]
  private readonly catalog: Record<string, CatalogProvider>

  constructor(script: (StreamChunk[] | 'hang')[], catalog: Record<string, CatalogProvider>) {
    super()
    this.script = script
    this.catalog = catalog
  }

  override providerInfo(provider: string) {
    const entry = this.catalog[provider]
    if (entry === undefined) throw new Error(`MockAdapter: unknown provider ${provider}`)
    return { id: provider, name: entry.name }
  }

  override listModels(provider: string) {
    const entry = this.catalog[provider]
    return Promise.resolve((entry?.models ?? []).map(model => ({ provider, ...model })))
  }

  async * stream(options: GenerateOptions): AsyncIterable<StreamChunk> {
    this.requests.push(options)
    const entry = this.script.shift()
    if (entry === undefined) throw new Error('MockAdapter: script exhausted')
    if (entry === 'hang') {
      yield { type: 'block-start', index: 0, blockType: 'text' }
      yield { type: 'text-delta', index: 0, text: 'partial' }
      await new Promise<void>((_resolve, reject) => {
        if (options.signal?.aborted) {
          reject(new Error('aborted'))
          return
        }
        options.signal?.addEventListener('abort', () => { reject(new Error('aborted')) }, { once: true })
      })
      return
    }
    for (const chunk of entry) {
      if (options.signal?.aborted) throw new Error('aborted')
      yield chunk
    }
  }
}

/** Scripted text response ending in a clean stop. */
export function textResponse(text: string): StreamChunk[] {
  return [
    { type: 'block-start', index: 0, blockType: 'text' },
    ...Array.from(text, (char): StreamChunk => ({ type: 'text-delta', index: 0, text: char })),
    { type: 'block-end', index: 0, block: { type: 'text', text } },
    { type: 'usage', usage: { inputTokens: 5, outputTokens: text.length } },
    { type: 'finish', reason: { kind: 'stop' } },
  ]
}

/**
 * The fixture preset roster: `alpha` and `beta`, each contributing one tool
 * named after itself, so a test can read a session's composition off its tool
 * list and see a recompose replace it.
 */
export const PRESET_FIXTURES = join(dirname(fileURLToPath(import.meta.url)), 'fixtures', 'presets')

/** Scripted response that fails after publishing an uncommitted partial chunk. */
export function errorResponse(message: string): StreamChunk[] {
  return [
    { type: 'block-start', index: 0, blockType: 'text' },
    { type: 'text-delta', index: 0, text: 'partial' },
    { type: 'finish', reason: { kind: 'error', failure: { message, code: 'PROVIDER_ERROR' } } },
  ]
}

export type CapturedUpdate = SessionNotification['update']

/**
 * One entry on the client-observed timeline. Session updates and permission
 * requests arrive over the same connection, so recording them together is the
 * only way to assert that a tool card precedes the prompt that references it.
 */
export type OrderedEvent =
  | { kind: 'update'; update: CapturedUpdate }
  | { kind: 'permission'; request: RequestPermissionRequest }

export interface BridgeHarness {
  ctx: Context
  client: ClientSideConnection
  adapter: MockAdapter
  updates: CapturedUpdate[]
  sessionUpdates: { sessionId: string; update: CapturedUpdate }[]
  permissionRequests: RequestPermissionRequest[]
  /** Updates and permission requests interleaved in arrival order. */
  orderedEvents: OrderedEvent[]
  onPermission: (request: RequestPermissionRequest) => RequestPermissionResponse
  closeClientTransport: () => Promise<void>
  acpFiber: Awaited<ReturnType<Context['plugin']>>
  /** The AgentLoop fiber, so a test can reload the loop out from under the bridge. */
  loopFiber: Awaited<ReturnType<Context['plugin']>>
  dispose: () => Promise<void>
}

type AcpConfigOverrides = { [K in keyof AcpConfig]?: AcpConfig[K] | undefined }

/** Build the bridge and a connected SDK client over cross-wired byte streams. */
export async function makeBridgeHarness(options: {
  script?: (StreamChunk[] | 'hang')[]
  config?: AcpConfigOverrides
  persona?: string
  /**
   * Mount the {@link PRESET_FIXTURES} roster with this default, so the bridge
   * composes sessions from a preset and offers the modes as a session config
   * option. Omitted, no roster is published and the bridge takes its no-modes
   * path.
   */
  presets?: { default: string }
  /**
   * Mount durable JSONL persistence under this root, so a session outlives its
   * agent and `session/load` has something to resume. Omitted, the deployment
   * persists nothing and the bridge takes its no-archive path.
   */
  persistenceRoot?: string
  /**
   * Provider routes and models the one scripted adapter serves, so a test can
   * exercise a model picker with something to pick. Every route runs the same
   * script, so a switch still produces a real turn. Defaults to the lone
   * `mock/mock` route.
   */
  catalog?: Record<string, CatalogProvider>
} = {}): Promise<BridgeHarness> {
  const catalog = options.catalog ?? DEFAULT_CATALOG
  const adapter = new MockAdapter(options.script ?? [], catalog)
  const ctx = new Context()
  await mountAgentLoopTestDependencies(ctx, { systemPrompt: { persona: options.persona ?? '' } })
  // Before the loop, as the app composes it: the coordinator captures a
  // session's header from `session/created`, which the first agent emits.
  if (options.persistenceRoot !== undefined) {
    await ctx.plugin(JsonlSessionPersistence, { root: options.persistenceRoot })
  }
  const loopFiber = await ctx.plugin(AgentLoop, { agents: [] })
  ctx.llm.registerAdapter(Object.keys(catalog), adapter)
  if (options.presets !== undefined) {
    // Before the bridge: it captures the roster during `apply`, exactly as the
    // app's composition orders them.
    ctx.baseUrl = pathToFileURL(PRESET_FIXTURES).href + '/'
    await ctx.plugin(Loader)
    await ctx.plugin(AgentPresets, {
      default: options.presets.default,
      roots: [{ path: PRESET_FIXTURES, trust: 'system' as const }],
      includeUserRoot: false,
    })
  }

  const agentToClient = new TransformStream<Uint8Array, Uint8Array>()
  const clientToAgent = new TransformStream<Uint8Array, Uint8Array>()
  const clientToAgentWriter = clientToAgent.writable.getWriter()
  const clientOutput = new WritableStream<Uint8Array>({
    write: chunk => clientToAgentWriter.write(chunk),
  })
  const agentStream: Stream = ndJsonStream(agentToClient.writable, clientToAgent.readable)
  const clientStream: Stream = ndJsonStream(clientOutput, agentToClient.readable)

  const updates: CapturedUpdate[] = []
  const sessionUpdates: { sessionId: string; update: CapturedUpdate }[] = []
  const permissionRequests: RequestPermissionRequest[] = []
  const orderedEvents: OrderedEvent[] = []
  const harness: BridgeHarness = {
    ctx,
    adapter,
    updates,
    sessionUpdates,
    permissionRequests,
    orderedEvents,
    onPermission: () => ({ outcome: { outcome: 'cancelled' } }),
    client: undefined as unknown as ClientSideConnection,
    acpFiber: undefined as unknown as BridgeHarness['acpFiber'],
    loopFiber,
    closeClientTransport: async () => { await clientToAgentWriter.close() },
    dispose: async () => { await ctx.fiber.dispose() },
  }

  const makeClient = (_agent: AcpAgent): Client => ({
    sessionUpdate(params: SessionNotification): Promise<void> {
      updates.push(params.update)
      sessionUpdates.push({ sessionId: params.sessionId, update: params.update })
      orderedEvents.push({ kind: 'update', update: params.update })
      return Promise.resolve()
    },
    requestPermission(params: RequestPermissionRequest): Promise<RequestPermissionResponse> {
      permissionRequests.push(params)
      orderedEvents.push({ kind: 'permission', request: params })
      return Promise.resolve(harness.onPermission(params))
    },
  })

  const config = { stream: agentStream, ...options.config } as AcpConfig
  if (!(options.config && 'provider' in options.config)) config.provider = 'mock'
  if (!(options.config && 'model' in options.config)) config.model = 'mock'
  harness.acpFiber = await ctx.plugin({
    name: 'acp-ide-test',
    inject: [...AcpPlugin.inject],
    apply: (inner: Context) => { AcpPlugin.apply(inner, config) },
  })
  harness.client = new ClientSideConnection(makeClient, clientStream)
  return harness
}

/**
 * Wait until the client has observed at least `count` timeline entries.
 * @param harness - the fixture whose client timeline is polled.
 * @param count - the number of entries to wait for.
 */
export async function waitForEvents(harness: BridgeHarness, count: number): Promise<void> {
  const deadline = Date.now() + 2000
  while (harness.orderedEvents.length < count) {
    if (Date.now() > deadline) {
      throw new Error(`timed out waiting for ${count} client events (saw ${harness.orderedEvents.length})`)
    }
    await new Promise(resolve => setTimeout(resolve, 5))
  }
}

/**
 * Wait until the client has observed `count` session updates of one kind.
 *
 * `waitForEvents` counts everything the session has produced, so a finished
 * prompt already satisfies it. That makes it useless for waiting on a
 * notification the bridge publishes off the back of an event rather than as
 * part of the prompt reply — the wait returns at once and the assertion races
 * the notification. Wait for the notification itself instead.
 * @param harness - the fixture whose updates are polled.
 * @param kind - the `sessionUpdate` discriminant to count.
 * @param count - the number of such updates to wait for.
 */
export async function waitForUpdates<K extends CapturedUpdate['sessionUpdate']>(
  harness: BridgeHarness, kind: K, count: number,
): Promise<void> {
  const deadline = Date.now() + 2000
  while (updatesOfKind(harness, kind).length < count) {
    if (Date.now() > deadline) {
      throw new Error(
        `timed out waiting for ${count} ${kind} update(s) `
        + `(saw ${updatesOfKind(harness, kind).length})`,
      )
    }
    await new Promise(resolve => setTimeout(resolve, 5))
  }
}

/**
 * Every published config option of one semantic category.
 *
 * A session publishes several pickers at once and the set is replaced whole on
 * every publication, so a test reads the one it means by category rather than
 * by position.
 * @param configOptions - a published option set, however it arrived.
 * @param category - the `category` to keep.
 * @returns the matching options, in publication order.
 */
export function optionsOfCategory(
  configOptions: SessionConfigOption[] | null | undefined, category: string,
): SessionConfigOption[] {
  return (configOptions ?? []).filter(option => option.category === category)
}

/**
 * The one option of a category, asserted to be a select so its values can be read.
 * @param configOptions - a published option set.
 * @param category - the `category` the option is expected under.
 * @returns that option.
 */
export function selectOption(
  configOptions: SessionConfigOption[] | null | undefined, category: string,
): Extract<SessionConfigOption, { type: 'select' }> {
  const [option, ...rest] = optionsOfCategory(configOptions, category)
  if (rest.length > 0 || option?.type !== 'select') {
    throw new Error(`expected one ${category} select option, got ${JSON.stringify(configOptions)}`)
  }
  return option
}

/**
 * Collect every session update of one kind that the client has observed.
 * @param harness - the fixture to read.
 * @param kind - the `sessionUpdate` discriminator to keep.
 * @returns the matching updates in arrival order.
 */
export function updatesOfKind<K extends CapturedUpdate['sessionUpdate']>(
  harness: BridgeHarness, kind: K,
): Extract<CapturedUpdate, { sessionUpdate: K }>[] {
  return harness.updates.filter(
    (update): update is Extract<CapturedUpdate, { sessionUpdate: K }> => update.sessionUpdate === kind,
  )
}
