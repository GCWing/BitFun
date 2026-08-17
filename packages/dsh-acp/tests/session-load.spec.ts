/**
 * Reopening a persisted conversation: `session/load` resumes the stored
 * session, replays its history to the client, and brings back the mode it ran
 * under — the three things an IDE loses when a reopen silently becomes a new
 * session.
 *
 * The conversation under test is seeded OUTSIDE the bridge and then left to go
 * cold, which is what a client restart actually looks like from the agent's
 * side: nothing about the session is still in memory when the load arrives.
 */

import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { randomUUID } from 'node:crypto'
import { afterEach, describe, expect, it } from 'vitest'
import { PROTOCOL_VERSION, RequestError, type SessionConfigOption } from '@agentclientprotocol/sdk'
import { createUserMessage } from '@deepseek-ai/dsh-llm'
import { SessionId } from '@deepseek-ai/dsh-session'
import {
  makeBridgeHarness, selectOption, textResponse, updatesOfKind, waitForUpdates, type BridgeHarness,
} from './harness.ts'

/**
 * The one mode option. A reopened session publishes a model picker alongside
 * it, so the mode is read by category rather than by position.
 */
function modeOption(configOptions: SessionConfigOption[] | null | undefined): Extract<
  SessionConfigOption, { type: 'select' }
> {
  return selectOption(configOptions, 'mode')
}

/** The text of every update of one kind, in arrival order. */
function textsOfKind(
  harness: BridgeHarness, kind: 'user_message_chunk' | 'agent_message_chunk',
): string[] {
  return updatesOfKind(harness, kind)
    .map(update => (update.content.type === 'text' ? update.content.text : ''))
}

/**
 * Seed one persisted conversation and let it go cold.
 * @param harness - the fixture whose registry and roster compose the agent.
 * @param cwd - the working directory the session is created in.
 * @param prompt - the user text the scripted model answers.
 * @returns the persisted session id, with no live agent left behind.
 */
async function seedColdSession(
  harness: BridgeHarness, cwd: string, prompt: string,
): Promise<string> {
  const sessionId = SessionId(randomUUID())
  const presets = harness.ctx.get('agentPresets')
  if (presets === undefined) throw new Error('the fixture roster is required to seed a session')
  const handle = await harness.ctx.agents.create({
    sessionId,
    meta: { cwd, agentPreset: 'alpha' },
    agentOptions: { provider: 'mock', model: 'mock' },
    setup: async (agentCtx) => void await presets.mount(agentCtx, 'alpha'),
  })
  handle.agent.followup(createUserMessage({
    content: [{ type: 'text', text: prompt }],
    source: { kind: 'user' },
  }))
  await handle.agent.whenIdle()
  // Disposal is what makes this a COLD load: the agent, its session, and the
  // loop state are gone, and only the durable log remains.
  await handle.dispose()
  return sessionId
}

describe('session/load', () => {
  let harness: BridgeHarness | undefined
  let root: string | undefined

  afterEach(async () => {
    await harness?.dispose()
    harness = undefined
    if (root !== undefined) await rm(root, { recursive: true, force: true })
    root = undefined
  })

  it('advertises the capability', async () => {
    harness = await makeBridgeHarness()
    const result = await harness.client.initialize({
      protocolVersion: PROTOCOL_VERSION, clientCapabilities: {},
    })

    expect(result.agentCapabilities?.loadSession).toBe(true)
  })

  it('resumes a cold session with its history, mode, and context', async () => {
    root = await mkdtemp(join(tmpdir(), 'dsh-acp-load-'))
    harness = await makeBridgeHarness({
      script: [textResponse('stored answer'), textResponse('second answer')],
      presets: { default: 'beta' },
      persistenceRoot: root,
    })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const cwd = process.cwd()
    const sessionId = await seedColdSession(harness, cwd, 'stored question')
    expect(harness.ctx.agents.get(SessionId(sessionId))).toBeUndefined()

    const loaded = await harness.client.loadSession({ sessionId, cwd, mcpServers: [] })

    // The transcript the client renders comes only from replay.
    await waitForUpdates(harness, 'agent_message_chunk', 1)
    expect(textsOfKind(harness, 'user_message_chunk')).toEqual(['stored question'])
    expect(textsOfKind(harness, 'agent_message_chunk')).toEqual(['stored answer'])
    // The stored preset wins over the roster default (`beta`), and a session
    // whose conversation has started comes back with the mode fixed.
    const option = modeOption(loaded.configOptions)
    expect(option.currentValue).toBe('alpha')
    expect(option.options).toEqual([
      { value: 'alpha', name: 'Alpha mode', description: 'the fixture default' },
    ])
    expect(option.description).toContain('already started')
    const resumed = harness.ctx.agents.get(SessionId(sessionId))
    expect(resumed).toBeDefined()
    expect(harness.ctx.tools.schemas(resumed!).map(schema => schema.name)).toEqual(['alpha'])

    // Context, not just presentation: the next prompt carries the stored turn.
    const response = await harness.client.prompt({
      sessionId, prompt: [{ type: 'text', text: 'follow-up' }],
    })

    expect(response.stopReason).toBe('end_turn')
    const request = harness.adapter.requests.at(-1)
    const sent = JSON.stringify(request?.messages ?? [])
    expect(sent).toContain('stored question')
    expect(sent).toContain('stored answer')
  })

  it('refuses a session it has never stored', async () => {
    root = await mkdtemp(join(tmpdir(), 'dsh-acp-load-'))
    harness = await makeBridgeHarness({ persistenceRoot: root })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })

    // Refused rather than answered with a blank session: a client that is told
    // "no such session" can fall back to session/new and know it did.
    await expect(harness.client.loadSession({
      sessionId: randomUUID(), cwd: process.cwd(), mcpServers: [],
    })).rejects.toThrow(RequestError)
  })

  it('refuses a session stored for another directory', async () => {
    root = await mkdtemp(join(tmpdir(), 'dsh-acp-load-'))
    harness = await makeBridgeHarness({
      script: [textResponse('stored answer')],
      presets: { default: 'alpha' },
      persistenceRoot: root,
    })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const sessionId = await seedColdSession(harness, process.cwd(), 'stored question')

    await expect(harness.client.loadSession({
      sessionId, cwd: tmpdir(), mcpServers: [],
    })).rejects.toThrow(RequestError)
  })
})
