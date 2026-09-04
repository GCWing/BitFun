/**
 * IDE-oriented Agent Client Protocol server over JSON-RPC stdio.
 *
 * Forked from `@deepseek-ai/dsh-acp`
 * (deepseek-harness/packages/acp/acp/src/index.ts, MIT).
 * Copyright (c) DeepSeek. See ../NOTICE.md.
 *
 * The upstream bridge is automation-only by design: it carries prompt text,
 * committed assistant text, cancellation, and one-shot permission decisions,
 * and deliberately keeps tool calls, reasoning, and plans off the wire as
 * "presentation or trace data". An IDE is exactly the client that needs that
 * presentation data, so this fork keeps upstream's session, turn, and teardown
 * machinery verbatim and replaces only the `session/event` dispatch with a full
 * translation to ACP session updates.
 *
 * @module @openbitfun/dsh-acp/bridge
 */

import type { Context } from '@deepseek-ai/cordis'
import { randomUUID } from 'node:crypto'
import { isAbsolute } from 'node:path'
import { Readable, Writable } from 'node:stream'
import Schema from '@deepseek-ai/schemastery'
import {
  createUserMessage, errorChain, type LlmModelInfo, type LlmProviderInfo,
} from '@deepseek-ai/dsh-llm'
import {
  AgentSideConnection,
  ndJsonStream,
  PROTOCOL_VERSION,
  RequestError,
  type Agent as AcpAgent,
  type AuthenticateRequest,
  type CancelNotification,
  type InitializeRequest,
  type InitializeResponse,
  type LoadSessionRequest,
  type LoadSessionResponse,
  type NewSessionRequest,
  type NewSessionResponse,
  type PlanEntry,
  type PromptRequest,
  type PromptResponse,
  type SessionConfigOption,
  type SessionConfigSelectGroup,
  type SessionConfigSelectOption,
  type SessionNotification,
  type SetSessionConfigOptionRequest,
  type SetSessionConfigOptionResponse,
  type StopReason,
  type Stream,
  type ToolCallUpdate,
} from '@agentclientprotocol/sdk'
import {
  installModelSelection, type Agent, type ModelSelection, type ModelSelectionRef,
} from '@deepseek-ai/dsh-agent'
// The roster itself is read through `ctx.get`; only the stored-preset resolver
// is a value here. This import also carries the `agent-preset/selected`
// session-event declaration the switch below logs and the resolver reads back.
import { resolveSessionPreset, type AgentPreset } from '@deepseek-ai/dsh-agent-presets'
import {
  SessionId, type SessionEvent, type SessionHeader, type TurnEndReason,
} from '@deepseek-ai/dsh-session'
// Side-effect type import: declaration-merges the approval waterfall answered below.
import type {} from '@deepseek-ai/dsh-user-approval'
import { acpPromptToText, promptHasUnsupportedContent, turnEndToStopReason } from './codec.ts'
import { parseToolArguments, toolKind, toolLocations, toolOutcome } from './tool-view.ts'

export const name = 'acp-ide'
/** The bridge creates and owns agents; every other concern is carried by the agent composition. */
export const inject = ['agents']

/**
 * The single continuable-subagent teardown the bridge needs. Declared
 * structurally so this package does not depend on the subagent seam for one
 * shutdown hook; an absent service means nothing continuable was materialized.
 */
interface ContinuableDrain {
  /**
   * Close admission below exact host-owned parents, then dispose only their
   * continuable descendants child-first.
   */
  drainContinuableDescendants(parents: readonly Agent[]): Promise<void>
}

/**
 * The deployment's default model selection, declared structurally for the same
 * reason as {@link ContinuableDrain}: the bridge reads it when its own config
 * pins no provider/model, but does not depend on the settings seam to exist. A
 * composition without the service falls back to this plugin's config.
 */
interface DefaultModelSource {
  /** The provider/model a new agent should start on, live at call time. */
  currentSelection(): { provider?: string; model?: string }
}

/**
 * The stored-session reads `session/load` needs, declared structurally for the
 * same reason as {@link ContinuableDrain}. A deployment composed without
 * durable persistence simply has nothing to load, and the bridge answers that
 * as a protocol error rather than depending on the storage seam to exist.
 */
interface SessionArchive {
  /**
   * One stored session's header and full event log. This is the authoritative
   * read rather than a listing: it waits for a just-disposed session's final
   * drain, which is exactly the session a client reopens first.
   */
  inspect(id: SessionId): Promise<{
    readonly meta: SessionHeader
    readonly events: readonly SessionEvent[]
  }>
}

/** Preserve invalid-parameter detail in the SDK wire error message. */
function invalidParams(detail: string): RequestError {
  return RequestError.invalidParams(undefined, detail)
}

/** Preserve failed-turn detail; plain handler errors become a generic wire internal error. */
function internalError(detail: string): RequestError {
  return RequestError.internalError(undefined, detail)
}

/** Plugin config: the provider/model selection used for each ACP-created agent. */
export interface AcpConfig {
  /** Provider route for created agents; omitted defers to the deployment default. */
  provider?: string
  /** Model name for created agents; omitted defers to the deployment default. */
  model?: string
  /**
   * Publish `agent_thought_chunk` updates for model reasoning. Clients that
   * render reasoning inline want this; leave it off for a quieter wire.
   * Defaults to `true`.
   */
  reasoning?: boolean
  /** Runtime-only transport override; production uses stdio. */
  stream?: Stream
}

export const Config: Schema<AcpConfig> = Schema.object({
  provider: Schema.string(),
  model: Schema.string(),
  reasoning: Schema.boolean().default(true),
})

/** What this bridge already told the client about one tool call. */
interface PublishedCall {
  title: string
  rawInput: unknown
}

/** Per-session protocol state. */
interface SessionRecord {
  agent: Agent
  /** Exact owned-agent disposer; resolves after registry, loop, and session teardown. */
  dispose: () => Promise<void>
  /**
   * `turn:step` keys whose committed assistant text was already streamed as
   * deltas, so the committed message is not published a second time.
   */
  streamedText: Set<string>
  /** The same de-duplication ledger for reasoning. */
  streamedReasoning: Set<string>
  /**
   * Tool calls this bridge has published a `tool_call` update for, so the
   * approval hook can tell a client-visible call from one it must announce.
   */
  published: Map<string, PublishedCall>
  /**
   * The agent preset this session is composed from, absent when the deployment
   * composes no roster. Tracked here so the mode picker can report what is
   * already running without re-reading the scope chain.
   */
  presetId: string | undefined
  /**
   * Tail of this session's `session/set_config_option` requests. Two of them
   * must not run concurrently: a preset switch's blank-session check has to see
   * the one ahead of it, and every reply carries the WHOLE option set, so an
   * out-of-order reply would re-render the picker from state that has moved on.
   */
  configSwitch: Promise<void> | undefined
  /** Whether the client has been told the mode is now fixed, so it is said once. */
  presetLockPublished: boolean
  /**
   * This session's live model override, installed into its agent's prompt
   * assembly and request routing. `current` is undefined until the client picks
   * a model, which leaves the agent on the provider/model it was created with.
   */
  modelSelection: ModelSelectionRef
  /**
   * The provider/model the agent was created with, captured at creation rather
   * than re-read: the deployment default is a live setting, and the picker must
   * report what THIS session runs on, not what the next one would.
   */
  modelSeed: { provider?: string; model?: string }
  /** In-flight prompt and its captured turn number for exact settlement. */
  inflight: {
    resolve: (reason: StopReason) => void
    reject: (error: Error) => void
    messageId: string
    turn: number | undefined
    /** The correlated turn's ending, set at turn/end and settled at whole-agent idle. */
    endReason: TurnEndReason | undefined
  } | undefined
}

/** Key a streaming ledger entry by the model round that produced it. */
function stepKey(turn: number, step: number): string {
  return `${turn}:${step}`
}

/**
 * The session config option the preset roster is offered under.
 *
 * ACP has no command channel to the agent — a client's slash menu inserts the
 * command as ordinary prompt text, so a mode chosen that way arrives glued to
 * the user's own message. A config option of category `mode` is the channel the
 * spec provides for exactly this: the client renders it as a picker next to its
 * composer and sends the choice on its own request.
 */
const PRESET_CONFIG_ID = 'agent-preset'

/**
 * Why the picker is down to one entry, in the client's own words.
 *
 * Doubles as the error a late `session/set_config_option` is refused with, so a
 * client that asks anyway is told the same thing its UI already showed.
 */
const PRESET_LOCKED = 'This conversation has already started, so its mode is fixed. '
  + 'Start a new session to pick another mode.'

/**
 * The session config option the model catalog is offered under.
 *
 * ACP 0.25 has no model state of its own — `session/set_model` and
 * `SessionModelState` do not exist in this schema version — so a model picker
 * IS a config option, distinguished from the mode picker only by its category.
 * Clients key off `category: 'model'` first and fall back to matching the id,
 * which is why the id is the bare word rather than something namespaced.
 */
const MODEL_CONFIG_ID = 'model'

/**
 * Encode one provider/model pair as a config option value.
 *
 * A value id is one opaque string, but a model is only identified by its pair,
 * and the same model id can be served by two providers. Joining them on the
 * first `/` is also what clients parse to label a value with its provider when
 * they render an ungrouped list: provider routes carry no `/`, model ids may.
 * @param provider - the registered provider route.
 * @param model - the provider-owned model id.
 * @returns the wire value.
 */
function modelValue(provider: string, model: string): string {
  return `${provider}/${model}`
}

/**
 * Split a config option value back into its provider/model pair.
 * @param value - a value id previously produced by {@link modelValue}.
 * @returns the pair, or undefined when the value carries no separator.
 */
function parseModelValue(value: string): { provider: string; model: string } | undefined {
  const cut = value.indexOf('/')
  if (cut <= 0 || cut === value.length - 1) return undefined
  return { provider: value.slice(0, cut), model: value.slice(cut + 1) }
}

/**
 * Whether a session may still change its preset.
 *
 * A started conversation's history was produced under its preset's tools, so
 * swapping now would leave logged tool calls the new composition cannot make.
 * The roster states the rule and leaves the check to its caller.
 * @param session - the agent's session.
 * @returns true while no turn has started.
 */
function sessionBlank(session: Agent['session']): boolean {
  return !session.events.some(event => event.type === 'turn/start')
}

/**
 * One roster entry as a selectable value of the mode option.
 * @param preset - the roster entry.
 * @returns its id as the value, with whatever prose the preset carries.
 */
function presetValue(preset: AgentPreset): SessionConfigSelectOption {
  return {
    value: preset.id,
    name: preset.name ?? preset.id,
    ...preset.description === undefined ? {} : { description: preset.description },
  }
}

/**
 * Mount the IDE-facing ACP server.
 * @param ctx - Cordis context carrying the agent factory and session events.
 * @param config - Initial provider/model selection and optional test transport.
 */
export function apply(ctx: Context, config: AcpConfig): void {
  // ACP handlers execute outside this plugin's injection scope, so capture the
  // injected service during apply rather than reading it lazily in a callback.
  const agents = ctx.agents
  // The preset roster is optional. A deployment that keeps every model-facing
  // plugin in its host composition has no modes to offer, and this bridge then
  // behaves exactly as it did before they existed.
  const presets = ctx.get('agentPresets')
  // Optional for the same reason: a composition whose model routing lives
  // elsewhere simply offers no model picker, and the bridge behaves as it did
  // before there was one. The catalog itself is read lazily, so an adapter
  // registered after this plugin still appears in the list.
  const llm = ctx.get('llm')
  const logger = ctx.logger
  const sessions = new Map<SessionId, SessionRecord>()
  const publishReasoning = config.reasoning ?? true
  let closed = false
  let conn: AgentSideConnection

  /** Return the bridge-owned record for an agent, rejecting same-id impostors. */
  const ownedRecord = (agent: Agent): SessionRecord | undefined => {
    const record = sessions.get(agent.session.id)
    return record?.agent === agent ? record : undefined
  }

  const assertOpen = (): void => {
    if (closed) throw internalError('the ACP bridge has been disposed')
  }

  const requireSession = (sessionId: SessionId): SessionRecord => {
    const record = sessions.get(sessionId)
    if (record === undefined) throw invalidParams(`unknown session: ${sessionId}`)
    return record
  }

  /** Send a protocol update without letting a disconnected client fail an agent turn. */
  const notify = (notification: SessionNotification): void => {
    /* v8 ignore next 3 -- only a transport write failure reaches this guard. */
    void conn.sessionUpdate(notification).catch((error: unknown) => {
      logger.warn(`acp: session/update failed: ${String(error)}`)
    })
  }

  /** Send one update for a bridge-owned session. */
  const update = (record: SessionRecord, payload: SessionNotification['update']): void => {
    notify({ sessionId: record.agent.session.id, update: payload })
  }

  /**
   * The session's mode picker: the roster as one `select` config option.
   *
   * Broken presets are withheld — the roster keeps them so their directory
   * stays visible to an author, but offering one would advertise a choice every
   * attempt refuses. A locked session is published with only the mode in force,
   * which is the client-generic way to say "this cannot be changed": a client
   * disables a picker it has nothing left to pick from, and an agent that does
   * allow mid-conversation switching keeps its full list and stays switchable.
   * @param record - the session.
   * @param locked - whether the mode is already fixed; read off the session by default.
   * @returns the one option, or nothing when this deployment composes no roster.
   */
  const presetOptions = async (
    record: SessionRecord, locked = !sessionBlank(record.agent.session),
  ): Promise<SessionConfigOption[]> => {
    if (presets === undefined) return []
    const roster = await presets.list()
    const current = record.presetId
    const offered = roster
      .filter(preset => preset.broken === undefined)
      .filter(preset => !locked || preset.id === current)
    const currentValue = current ?? offered[0]?.id
    if (currentValue === undefined || offered.length === 0) return []
    return [{
      type: 'select',
      id: PRESET_CONFIG_ID,
      name: 'Mode',
      category: 'mode',
      ...locked ? { description: PRESET_LOCKED } : {},
      currentValue,
      options: offered.map(presetValue),
    }]
  }

  /**
   * The provider/model this session is running on right now.
   *
   * Three sources in falling order of authority: an explicit pick, the request
   * header the session's own turns were logged under, and the selection the
   * agent was created with. A resumed session therefore reports what it
   * actually ran on rather than what the deployment default has since become.
   * @param record - the session.
   * @returns the pair, or undefined when nothing pins one.
   */
  const currentModel = (record: SessionRecord): { provider: string; model: string } | undefined => {
    const picked = record.modelSelection.current
    if (picked !== undefined) return { provider: picked.provider, model: picked.model }
    const logged = record.agent.session.requestHeader()?.config
    if (logged !== undefined) return { provider: logged.provider, model: logged.model }
    const { provider, model } = record.modelSeed
    if (provider === undefined || model === undefined) return undefined
    return { provider, model }
  }

  /**
   * The session's model picker: the harness's model catalog as one `select`
   * config option, grouped by provider, one row per model.
   *
   * Unlike the mode, the model is switchable for the whole life of a session —
   * a mid-conversation switch changes only which model answers the next step,
   * and leaves the logged history valid. The catalog is advisory (an adapter
   * may accept ids it does not advertise), so the model in force is offered
   * even when it is not listed: a picker whose current value is missing from
   * its own options renders blank.
   *
   * A model reachable through two providers is listed once. The harness's own
   * DeepSeek adapter and a pi-ai profile pointed at the same account are both
   * routes to the same models, and a user who configured the second in dsh gets
   * every model twice — same name, same vendor, nothing to choose between. The
   * provider carrying the session's current model owns those rows, so the list
   * stays on the route the session is actually running.
   * @param record - the session.
   * @returns the one option, or nothing when no model can be named.
   */
  const modelOptions = async (record: SessionRecord): Promise<SessionConfigOption[]> => {
    if (llm === undefined) return []
    const current = currentModel(record)
    if (current === undefined) return []
    const currentValue = modelValue(current.provider, current.model)

    const listed: { provider: LlmProviderInfo; models: LlmModelInfo[] }[] = []
    for (const provider of llm.listProviders()) {
      // One unreachable provider costs its own entries, not the whole picker.
      const models = await llm.listModels(provider.id).catch((error: unknown) => {
        logger.warn(`acp: cannot list models for "${provider.id}": ${String(error)}`)
        return []
      })
      listed.push({ provider, models })
    }

    // Which provider each model id is listed under. Seeded with the session's
    // own route so the model in force keeps its provider even when that
    // provider's listing failed, then filled current-provider-first so a
    // duplicated catalog collapses onto the route already in use.
    const owner = new Map<string, string>([[current.model, current.provider]])
    const byCurrentFirst = [
      ...listed.filter(entry => entry.provider.id === current.provider),
      ...listed.filter(entry => entry.provider.id !== current.provider),
    ]
    for (const { provider, models } of byCurrentFirst) {
      for (const model of models) {
        if (!owner.has(model.id)) owner.set(model.id, provider.id)
      }
    }

    const groups: SessionConfigSelectGroup[] = []
    for (const { provider, models } of listed) {
      const values: SessionConfigSelectOption[] = models
        .filter(model => owner.get(model.id) === provider.id)
        .map(model => ({
          value: modelValue(provider.id, model.id),
          name: model.name,
          ...model.description === undefined ? {} : { description: model.description },
        }))
      if (provider.id === current.provider && !values.some(value => value.value === currentValue)) {
        values.push({ value: currentValue, name: current.model })
      }
      if (values.length === 0) continue
      groups.push({ group: provider.id, name: provider.name, options: values })
    }
    if (!groups.some(group => group.group === current.provider)) {
      groups.push({
        group: current.provider,
        name: current.provider,
        options: [{ value: currentValue, name: current.model }],
      })
    }
    return [{ type: 'select', id: MODEL_CONFIG_ID, name: 'Model', category: 'model', currentValue, options: groups }]
  }

  /**
   * Everything this session lets the client configure.
   *
   * Always published as one set, because that is how a client consumes it: a
   * `session/new` response, a `set_config_option` reply, and a
   * `config_option_update` each REPLACE the picker row wholesale, so an answer
   * that carried only the option that changed would drop the other one.
   * @param record - the session.
   * @param locked - whether the mode is already fixed; read off the session by default.
   * @returns the options, in the order a composer renders them.
   */
  const sessionOptions = async (
    record: SessionRecord, locked?: boolean,
  ): Promise<SessionConfigOption[]> => [
    ...await presetOptions(record, locked),
    ...await modelOptions(record),
  ]

  const settlePrompt = (record: SessionRecord, reason: StopReason): void => {
    const inflight = record.inflight
    if (inflight === undefined) return
    record.inflight = undefined
    inflight.resolve(reason)
  }

  const rejectFromError = (
    inflight: NonNullable<SessionRecord['inflight']>,
    reason: Extract<TurnEndReason, { kind: 'error' }>,
  ): void => {
    inflight.reject(internalError(`turn failed: ${reason.error.message}`))
  }

  /**
   * Announce a tool call to the client. Idempotent per call id so the approval
   * hook can safely re-announce a call whose `tool/call` event it did not see.
   */
  const publishToolCall = (
    record: SessionRecord, callId: string, toolName: string, rawInput: unknown,
  ): PublishedCall => {
    const existing = record.published.get(callId)
    if (existing !== undefined) return existing
    const published: PublishedCall = { title: toolName, rawInput }
    record.published.set(callId, published)
    update(record, {
      sessionUpdate: 'tool_call',
      toolCallId: callId,
      // The harness tool name IS the title: a client matches it against its own
      // tool vocabulary to render a native card, and a prettified label would
      // only make that match guess.
      title: toolName,
      kind: toolKind(toolName),
      status: 'pending',
      rawInput,
      locations: toolLocations(rawInput),
    })
    return published
  }

  /**
   * Translate one harness session event to the ACP wire. Everything the
   * upstream automation bridge withholds — streaming text, reasoning, tool
   * calls, tool results, and plans — is published here.
   */
  const publishEvent = (record: SessionRecord, event: SessionEvent): void => {
    switch (event.type) {
      case 'assistant/chunk': {
        const { turn, step, chunk } = event.data
        const key = stepKey(turn, step)
        if (chunk.type === 'text-delta') {
          if (chunk.text.length === 0) return
          record.streamedText.add(key)
          update(record, { sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: chunk.text } })
        } else if (chunk.type === 'reasoning-delta') {
          if (chunk.text.length === 0) return
          record.streamedReasoning.add(key)
          if (publishReasoning) {
            update(record, { sessionUpdate: 'agent_thought_chunk', content: { type: 'text', text: chunk.text } })
          }
        }
        return
      }

      case 'assistant/message': {
        const { turn, step, message } = event.data
        const key = stepKey(turn, step)
        // A step that streamed deltas already published this text; only a
        // non-streaming route (or a replayed round) reaches the client here.
        const emitText = !record.streamedText.has(key)
        const emitReasoning = publishReasoning && !record.streamedReasoning.has(key)
        for (const block of message.content) {
          if (block.type === 'text' && block.text.length > 0 && emitText) {
            update(record, { sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: block.text } })
          } else if (block.type === 'reasoning' && block.text.length > 0 && emitReasoning) {
            update(record, { sessionUpdate: 'agent_thought_chunk', content: { type: 'text', text: block.text } })
          } else if (block.type === 'image') {
            update(record, {
              sessionUpdate: 'agent_message_chunk',
              content: { type: 'text', text: `[image attachment ${block.attachment.attachmentId}]` },
            })
          }
        }
        return
      }

      case 'tool/call': {
        const { callId, name: toolName, arguments: args } = event.data
        publishToolCall(record, callId, toolName, parseToolArguments(args))
        return
      }

      case 'tool/result': {
        const outcome = toolOutcome(event.data.message, event.data.error !== undefined)
        if (outcome.callId === undefined) return
        record.published.delete(outcome.callId)
        update(record, {
          sessionUpdate: 'tool_call_update',
          toolCallId: outcome.callId,
          status: outcome.isError ? 'failed' : 'completed',
          content: outcome.content,
          rawOutput: event.data.message as unknown,
        })
        return
      }

      case 'todo/write': {
        const entries: PlanEntry[] = event.data.todos.map(todo => ({
          content: todo.content,
          // The harness todo list carries no priority; ACP requires one, and a
          // flat `medium` is the honest rendering of "unranked".
          priority: 'medium',
          status: todo.status,
        }))
        update(record, { sessionUpdate: 'plan', entries })
        return
      }

      case 'turn/start': {
        // The first turn fixes the mode: from here on the history was produced
        // under this composition's tools, and a later swap would leave logged
        // tool calls the new composition cannot make. Republishing the picker
        // shrunk to the mode in force is how the client learns that — said once,
        // because every later turn would say the same thing.
        if (presets === undefined || record.presetLockPublished) return
        record.presetLockPublished = true
        // Locked is asserted rather than read: whether this very event is
        // already in `session.events` is the session's business, not the
        // bridge's, and the answer here is known.
        void sessionOptions(record, true).then(configOptions => {
          if (closed || configOptions.length === 0) return
          if (sessions.get(record.agent.session.id) !== record) return
          update(record, { sessionUpdate: 'config_option_update', configOptions })
        }, (error: unknown) => {
          logger.warn(`acp: failed to publish the fixed agent preset: ${String(error)}`)
        })
        return
      }

      case 'step/end': {
        const key = stepKey(event.data.turn, event.data.step)
        record.streamedText.delete(key)
        record.streamedReasoning.delete(key)
        return
      }

      case 'turn/end': {
        record.streamedText.clear()
        record.streamedReasoning.clear()
        record.published.clear()
        return
      }

      default:
        // Every other harness event is internal bookkeeping with no ACP
        // counterpart; a client reconstructs round structure from the updates
        // above.
        return
    }
  }

  /**
   * Republish a resumed session's history so the client renders the
   * conversation it is reopening.
   *
   * Replay is the client's only source for a loaded session: `session/load`
   * returns no transcript, so everything the IDE shows above the composer comes
   * from these updates. What is withheld is what would double up or lie —
   * `assistant/chunk` deltas are already committed into `assistant/message`,
   * and `turn/start` would announce a mode lock the load response itself
   * carries. Non-user `user/message` entries (plugin reminders, tool feedback)
   * are agent-internal input, not something the user typed.
   * @param record - the resumed session, already registered.
   */
  const replayHistory = (record: SessionRecord): void => {
    for (const event of record.agent.session.events) {
      try {
        if (event.type === 'user/message') {
          if (event.data.source.kind !== 'user') continue
          for (const block of event.data.content) {
            if (block.type !== 'text' || block.text.length === 0) continue
            update(record, { sessionUpdate: 'user_message_chunk', content: { type: 'text', text: block.text } })
          }
          continue
        }
        if (event.type === 'assistant/chunk' || event.type === 'turn/start') continue
        publishEvent(record, event)
      } catch (error: unknown) {
        // One unrenderable historical event costs its card, not the reopened
        // conversation.
        logger.warn(`acp: failed to replay ${event.type}: ${String(error)}`)
      }
    }
  }

  ctx.on('session/event', (session, event: SessionEvent) => {
    const record = sessions.get(session.header.id)
    if (record === undefined || record.agent.session !== session) return
    try {
      publishEvent(record, event)
    } catch (error: unknown) {
      // A presentation-mapping failure must never break turn settlement below:
      // the client loses one card, not the answer.
      logger.warn(`acp: failed to publish ${event.type}: ${String(error)}`)
    } finally {
      const inflight = record.inflight
      if (inflight !== undefined && event.type === 'turn/end' && inflight.turn === event.data.turn) {
        if (event.data.reason.kind === 'error') {
          // Model failures surface immediately as prompt errors; ordinary
          // endings wait for whole-agent idle below.
          record.inflight = undefined
          rejectFromError(inflight, event.data.reason)
        } else {
          inflight.endReason = event.data.reason
        }
      }
    }
  })

  ctx.on('agent/inbox/claimed', ({ agent, message, turn }) => {
    const record = ownedRecord(agent)
    const inflight = record?.inflight
    if (inflight !== undefined && inflight.messageId === message.id) inflight.turn = turn
  })

  ctx.on('agent/error', ({ agent, turn, error }) => {
    const record = ownedRecord(agent)
    const inflight = record?.inflight
    if (record === undefined || inflight === undefined || inflight.turn === turn) return
    record.inflight = undefined
    inflight.reject(internalError(`turn failed: ${errorChain(error)}`))
  })

  // Permission requests reach the client attached to a tool call it has already
  // been shown. The harness appends `tool/call` before the scheduler prepares
  // the call (agent-loop `tool-calls.ts`), so the card exists by the time this
  // runs; `publishToolCall` re-announces it anyway rather than trusting that
  // ordering, because a permission prompt against an unknown call id is exactly
  // the failure that leaves an IDE waiting for a card that never arrives.
  ctx.on('approval/request', (request, next) => {
    const record = ownedRecord(request.agent)
    if (record === undefined || request.callId === undefined) return next()
    const callId = request.callId
    const published = publishToolCall(record, callId, request.toolName, {})
    const toolCall: ToolCallUpdate = {
      toolCallId: callId,
      title: published.title,
      kind: toolKind(published.title),
      status: 'pending',
      rawInput: published.rawInput,
    }
    return conn.requestPermission({
      sessionId: record.agent.session.id,
      toolCall,
      options: [
        { optionId: 'allow-once', name: 'Allow once', kind: 'allow_once' },
        { optionId: 'reject-once', name: 'Reject', kind: 'reject_once' },
      ],
    }).then(({ outcome }) => {
      if (outcome.outcome === 'cancelled') return 'cancelled'
      if (outcome.optionId !== 'allow-once') return 'rejected'
      // Granted: the call is about to run, so move the card out of `pending`.
      // `tool/result` settles it to completed or failed.
      update(record, { sessionUpdate: 'tool_call_update', toolCallId: callId, status: 'in_progress' })
      return 'allowed-once'
    })
  })

  const makeAgent = (connection: AgentSideConnection): AcpAgent => {
    conn = connection
    return {
      initialize(_params: InitializeRequest): Promise<InitializeResponse> {
        // Single-version agent: the spec's "same version if supported, else
        // the latest supported" both resolve to this server's one version.
        return Promise.resolve({
          protocolVersion: PROTOCOL_VERSION,
          agentInfo: { name: 'dsh-acp-ide', version: '0.0.1' },
          agentCapabilities: {
            // Reopening a conversation is the IDE's normal case, and the
            // harness persists every session it runs: without this, a client
            // has no way to say "continue that one" and starts a blank session
            // whose history and mode are gone.
            loadSession: true,
            promptCapabilities: { image: false, audio: false, embeddedContext: false },
          },
          authMethods: [],
        })
      },

      authenticate(_params: AuthenticateRequest): Promise<void> {
        return Promise.resolve()
      },

      async newSession(params: NewSessionRequest): Promise<NewSessionResponse> {
        assertOpen()
        validateSessionParams(params)
        const sessionId = SessionId(randomUUID())
        // Resolved before creation so the session header records the exact id
        // that was composed, not the alias `undefined` stands for: the roster's
        // default is a hot-reloaded setting, and a resumed session must rebuild
        // the composition its turns actually ran under.
        const composed = presets === undefined ? undefined : (await presets.resolve()).id
        const modelSeed = agentOptions(ctx, config)
        const modelSelection: ModelSelectionRef = { current: undefined, assembled: undefined }
        const handle = await agents.create({
          sessionId,
          meta: { cwd: params.cwd, ...composed === undefined ? {} : { agentPreset: composed } },
          agentOptions: modelSeed,
          // Composition belongs in `setup`, which the factory awaits before the
          // agent is published: a preset that fails to mount rolls the whole
          // creation back rather than yielding a session on the empty global
          // tool layer.
          setup: async (agentCtx: Context) => {
            installModelSelection(agentCtx, modelSelection)
            if (presets !== undefined && composed !== undefined) await presets.mount(agentCtx, composed)
          },
        })
        /* v8 ignore next 4 -- a real stdio close can race an in-flight create. */
        if (closed) {
          await handle.dispose()
          throw internalError('connection closed during session/new')
        }
        const record: SessionRecord = {
          agent: handle.agent,
          dispose: () => handle.dispose(),
          streamedText: new Set(),
          streamedReasoning: new Set(),
          published: new Map(),
          presetId: composed,
          configSwitch: undefined,
          presetLockPublished: false,
          modelSelection,
          modelSeed,
          inflight: undefined,
        }
        sessions.set(sessionId, record)
        // Carried by the response rather than announced afterwards: a client
        // registers its update handler for a session only once `session/new`
        // has returned, so anything published before that can be dropped.
        const configOptions = await sessionOptions(record, false)
        return { sessionId, ...configOptions.length === 0 ? {} : { configOptions } }
      },

      /**
       * Reopen a persisted session: resume its agent and replay its history.
       *
       * The session is rebuilt from storage under the preset its turns actually
       * ran on ({@link resolveSessionPreset} reads the log, not just the
       * creation header), because a resumed conversation's tool calls have to
       * stay ones the mounted composition can still make. That also settles the
       * mode picker: a session with turns comes back locked, exactly as it was
       * when the client last saw it.
       *
       * An unknown id is refused with an invalid-parameter error rather than
       * silently answered with an empty session, so a client can fall back to
       * `session/new` and know that is what happened.
       */
      async loadSession(params: LoadSessionRequest): Promise<LoadSessionResponse> {
        assertOpen()
        validateSessionParams(params)
        const sessionId = SessionId(params.sessionId)
        // Already live — a second load of the same session must not resume a
        // rival agent on top of the running one. Replaying is still right: the
        // client asked for the transcript.
        const live = sessions.get(sessionId)
        if (live !== undefined) {
          replayHistory(live)
          const configOptions = await sessionOptions(live)
          return configOptions.length === 0 ? {} : { configOptions }
        }

        const archive = ctx.get('sessionPersistence') as SessionArchive | undefined
        if (archive === undefined) throw invalidParams('this deployment persists no sessions')
        // Absent, unreadable, and written-by-an-incompatible-format all mean the
        // same thing to a client — this conversation cannot be reopened — and
        // saying so as an invalid parameter is what lets it fall back to
        // `session/new` knowing why.
        const inspected = await archive.inspect(sessionId).catch((error: unknown) => {
          throw invalidParams(`cannot load session ${sessionId}: ${errorChain(error)}`)
        })
        if (inspected.meta.cwd !== params.cwd) {
          throw invalidParams(
            `session ${sessionId} belongs to "${inspected.meta.cwd ?? '(none)'}", not "${params.cwd}"`,
          )
        }
        const composed = presets === undefined
          ? undefined
          : resolveSessionPreset({ header: inspected.meta, events: inspected.events })
        const modelSeed = agentOptions(ctx, config)
        const modelSelection: ModelSelectionRef = { current: undefined, assembled: undefined }
        const handle = await agents.resume({
          resumeSessionId: sessionId,
          agentOptions: modelSeed,
          setup: async (agentCtx: Context) => {
            installModelSelection(agentCtx, modelSelection)
            if (presets !== undefined && composed !== undefined) await presets.mount(agentCtx, composed)
          },
        })
        /* v8 ignore next 4 -- a real stdio close can race an in-flight resume. */
        if (closed) {
          await handle.dispose()
          throw internalError('connection closed during session/load')
        }
        const record: SessionRecord = {
          agent: handle.agent,
          dispose: () => handle.dispose(),
          streamedText: new Set(),
          streamedReasoning: new Set(),
          published: new Map(),
          presetId: composed,
          configSwitch: undefined,
          // A session with turns is already locked, and the response below says
          // so; only a session reopened before its first turn can still be
          // told, by the `turn/start` that starts it.
          presetLockPublished: !sessionBlank(handle.agent.session),
          modelSelection,
          modelSeed,
          inflight: undefined,
        }
        sessions.set(sessionId, record)
        replayHistory(record)
        const configOptions = await sessionOptions(record)
        return configOptions.length === 0 ? {} : { configOptions }
      },

      /**
       * Choose the session's mode or its model.
       *
       * A mode switch is refused once the conversation has started, for the same
       * reason the picker is down to one entry by then: the turns already logged
       * ran on the current preset's tools. A model switch is not — swapping which
       * model answers the next step leaves every logged turn valid — so the model
       * picker stays live for the whole session.
       */
      async setSessionConfigOption(
        params: SetSessionConfigOptionRequest,
      ): Promise<SetSessionConfigOptionResponse> {
        assertOpen()
        const record = requireSession(SessionId(params.sessionId))
        if (params.configId !== PRESET_CONFIG_ID && params.configId !== MODEL_CONFIG_ID) {
          throw invalidParams(`unknown config option: ${params.configId}`)
        }
        if (params.configId === MODEL_CONFIG_ID) {
          if (llm === undefined) throw invalidParams('this deployment offers no models')
          if (typeof params.value !== 'string') {
            throw invalidParams(`"${MODEL_CONFIG_ID}" is a select option, not a boolean`)
          }
          const value = params.value
          const pair = parseModelValue(value)
          if (pair === undefined) throw invalidParams(`unknown model: ${value}`)
          // Resolved rather than trusted: this both rejects a route the harness
          // cannot serve — before it becomes a failing turn — and materializes
          // the adapter's own defaults, so a model whose provider configures a
          // reasoning effort runs at that effort instead of the previous
          // model's inherited one.
          const resolved = await llm.resolveCallConfig(pair).catch((error: unknown) => {
            throw invalidParams(`cannot use model "${value}": ${errorChain(error)}`)
          })
          const selection: ModelSelection = {
            provider: resolved.provider,
            model: resolved.model,
            ...resolved.reasoningEffort === undefined ? {} : { reasoningEffort: resolved.reasoningEffort },
          }
          const pick = async (): Promise<SessionConfigOption[]> => {
            record.modelSelection.current = selection
            return await sessionOptions(record)
          }
          const queuedPick = (record.configSwitch ?? Promise.resolve()).then(pick)
          record.configSwitch = queuedPick.then(() => undefined, () => undefined)
          return { configOptions: await queuedPick }
        }
        if (presets === undefined) throw invalidParams('this deployment offers no modes')
        if (typeof params.value !== 'string') {
          throw invalidParams(`"${PRESET_CONFIG_ID}" is a select option, not a boolean`)
        }
        const id = params.value
        const swap = async (): Promise<SessionConfigOption[]> => {
          const target = (await presets.list()).find(preset => preset.id === id)
          if (target === undefined) throw invalidParams(`unknown mode: ${id}`)
          if (target.broken !== undefined) {
            throw invalidParams(`mode "${id}" cannot be used: ${target.broken}`)
          }
          // Asking for the mode already in force is not a switch, so it is
          // answered even on a locked session: it changes nothing either way.
          if (record.presetId === target.id) return await sessionOptions(record)
          // Re-read inside the queue: a switch ahead of this one may have run,
          // and a conversation may have started, since this request arrived.
          if (!sessionBlank(record.agent.session)) throw invalidParams(PRESET_LOCKED)
          try {
            const preset = await presets.recompose(record.agent.ctx, target.id)
            // Logged only after the swap committed, and appended rather than
            // written back into the creation header: every later turn runs under
            // this composition, so a resumed session must rebuild it rather than
            // the one the session was created with.
            record.agent.session.append('agent-preset/selected', { agentPreset: preset.id })
            record.presetId = preset.id
          } catch (error: unknown) {
            // A rejected recompose leaves the previous composition mounted, so
            // the session stays usable and the user can pick again.
            throw internalError(`failed to switch to "${id}": ${errorChain(error)}`)
          }
          return await sessionOptions(record)
        }
        const queued = record.configSwitch ?? Promise.resolve()
        const turn = queued.then(swap)
        record.configSwitch = turn.then(() => undefined, () => undefined)
        return { configOptions: await turn }
      },

      async prompt(params: PromptRequest): Promise<PromptResponse> {
        assertOpen()
        const record = requireSession(SessionId(params.sessionId))
        if (record.inflight !== undefined) {
          throw invalidParams('a prompt is already in flight for this session')
        }
        if (promptHasUnsupportedContent(params.prompt)) {
          throw invalidParams('only text and resource_link prompt content is supported')
        }
        const text = acpPromptToText(params.prompt)
        if (text.trim().length === 0) throw invalidParams('empty prompt')

        // Not driving a retired agent is this bridge's contract: an
        // agent-loop-only reload disposes the loop's agents while the bridge
        // record survives, so validate the record against the live registry
        // before sending — a disposed machine would accept the item silently.
        if (ctx.agents.get(record.agent.id) !== record.agent) {
          throw internalError('prompt was not queued: the agent was disposed outside the bridge')
        }
        const message = createUserMessage({ content: [{ type: 'text', text }], source: { kind: 'user' } })
        const stopReason = await new Promise<StopReason>((resolve, reject) => {
          // Arm the slot before followup() so a listener-driven synchronous
          // turn cannot slip past correlation; a synchronous followup()
          // failure (invalid input) must free the slot again or the session
          // would reject every later prompt as already in flight.
          const inflight: NonNullable<SessionRecord['inflight']> = {
            resolve, reject, messageId: message.id, turn: undefined, endReason: undefined,
          }
          record.inflight = inflight
          try {
            record.agent.followup(message)
            /* v8 ignore start -- future-proofing guard, see above */
          } catch (error: unknown) {
            record.inflight = undefined
            const detail = error instanceof Error ? error.message : String(error)
            throw internalError(`prompt was not queued: ${detail}`)
          }
          /* v8 ignore stop */
          // Settlement waits for whole-agent idle: a correlated turn/end arms
          // `endReason`, while a turnless slot (admission discarded the
          // prompt) stays cancelled. Other producers may run further turns
          // before quiescence; the prompt settles only when the agent stops.
          void record.agent.whenIdle().then(() => {
            if (record.inflight !== inflight) return
            record.inflight = undefined
            const end = inflight.endReason
            if (end === undefined) {
              inflight.resolve('cancelled')
            } else {
              // Token-limit and other non-terminal endings are not prompt-level
              // stop reasons; only normal quiescence reports end_turn.
              inflight.resolve(end.kind === 'max-tokens' ? 'end_turn' : turnEndToStopReason(end))
            }
          })
        })
        return { stopReason }
      },

      cancel(params: CancelNotification): Promise<void> {
        const record = sessions.get(SessionId(params.sessionId))
        if (record === undefined) return Promise.resolve()
        record.agent.cancel({ kind: 'user' })
        settlePrompt(record, 'cancelled')
        return Promise.resolve()
      },
    }
  }

  /* v8 ignore next 4 -- production stdio wiring; tests inject config.stream. */
  const stream: Stream = config.stream ?? ndJsonStream(
    Writable.toWeb(process.stdout) as WritableStream<Uint8Array>,
    Readable.toWeb(process.stdin) as ReadableStream<Uint8Array>,
  )
  conn = new AgentSideConnection(makeAgent, stream)

  let quiescing: Promise<void> | undefined
  const quiesce = (): Promise<void> => {
    if (quiescing !== undefined) return quiescing
    closed = true
    const records = [...sessions.values()]
    sessions.clear()
    // Stop the bridge's own work before any await: a descendant drain can block
    // on persistence or scoped cleanup, and the top-level agents must not keep
    // running model and tool calls for its whole duration.
    for (const record of records) {
      record.agent.cancel({ kind: 'user' })
      settlePrompt(record, 'cancelled')
    }
    quiescing = (async () => {
      // Continuable subagents outlive the turn that started them, and their
      // Activations own descendant teardown. Drain only these sessions' forests
      // child-first BEFORE disposing the top-level agents, so no descendant is
      // left holding a runtime its owner already released.
      const subagents = ctx.get('subagents') as ContinuableDrain | undefined
      if (subagents !== undefined) {
        try {
          await subagents.drainContinuableDescendants(records.map(record => record.agent))
        } catch (error: unknown) {
          logger.warn(`acp: continuable subagent teardown failed: ${String(error)}`)
        }
      }
      const disposals = await Promise.allSettled(records.map(record => record.dispose()))
      const failures: unknown[] = []
      for (const result of disposals) {
        if (result.status === 'rejected') failures.push(result.reason as unknown)
      }
      if (failures.length > 0) {
        const detail = failures.map(failure => errorChain(failure)).join('; ')
        throw new AggregateError(
          failures,
          `ACP agent teardown failed for ${failures.length} session(s): ${detail}`,
        )
      }
    })()
    return quiescing
  }

  /* v8 ignore start -- production transport rejection and teardown failure. */
  void conn.closed
    .catch((error: unknown) => {
      logger.warn(`acp: connection closed with an error: ${String(error)}`)
    })
    .then(quiesce)
    .catch((error: unknown) => {
      logger.warn(`acp: connection-close teardown failed: ${String(error)}`)
    })
  /* v8 ignore stop */

  ctx.effect(() => quiesce, 'acp.connection')
}

/**
 * Build per-agent options without assigning absent optional fields.
 *
 * Plugin config wins where it is set; anything it leaves open comes from the
 * deployment's default selection, read HERE rather than captured at `apply` so
 * that a model chosen in the harness's settings after this process started
 * takes effect on the next session.
 * @param ctx - the plugin context, source of the optional default-model service.
 * @param config - ACP provider/model configuration.
 * @returns the resolved fields only.
 */
function agentOptions(ctx: Context, config: AcpConfig): { provider?: string; model?: string } {
  const fallback = (ctx.get('agentDefaultModel') as DefaultModelSource | undefined)?.currentSelection()
  const provider = config.provider ?? fallback?.provider
  const model = config.model ?? fallback?.model
  return {
    ...provider !== undefined ? { provider } : {},
    ...model !== undefined ? { model } : {},
  }
}

/** Reject session features outside this bridge's contract. */
function validateSessionParams(params: NewSessionRequest | LoadSessionRequest): void {
  if (!isAbsolute(params.cwd)) throw invalidParams(`cwd must be an absolute path: ${params.cwd}`)
  if (params.additionalDirectories !== undefined && params.additionalDirectories.length > 0) {
    throw invalidParams('additionalDirectories is not supported')
  }
  if (params.mcpServers.length > 0) throw invalidParams('mcpServers is not supported')
}
