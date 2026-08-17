/**
 * Modes: the preset roster a session is composed from, offered to the client as
 * a `mode` session config option and fixed once the conversation starts.
 *
 * The switch is driven the way a client's composer picker drives it — through
 * `session/set_config_option` — and the result is read off the session's tool
 * list rather than off anything the bridge reports about itself.
 */

import { afterEach, describe, expect, it } from 'vitest'
import { PROTOCOL_VERSION, type SessionConfigOption } from '@agentclientprotocol/sdk'
import { SessionId } from '@deepseek-ai/dsh-session'
import {
  makeBridgeHarness, optionsOfCategory, selectOption, textResponse, updatesOfKind, waitForUpdates,
  type BridgeHarness,
} from './harness.ts'

/** The tools a session's model can call, i.e. what its preset composed. */
function toolNames(harness: BridgeHarness, sessionId: string): string[] {
  const agent = harness.ctx.agents.get(SessionId(sessionId))
  if (agent === undefined) throw new Error(`no agent for session ${sessionId}`)
  return harness.ctx.tools.schemas(agent).map(schema => schema.name).sort()
}

/** Every preset the session has been switched to, in order. */
function selections(harness: BridgeHarness, sessionId: string): unknown[] {
  const agent = harness.ctx.agents.get(SessionId(sessionId))
  if (agent === undefined) throw new Error(`no agent for session ${sessionId}`)
  return agent.session.events
    .filter(event => event.type === 'agent-preset/selected')
    .map(event => (event as { data: { agentPreset: string } }).data.agentPreset)
}

/**
 * The one mode option. A session publishes a model picker alongside it, so the
 * mode is read by category rather than by being the only thing on the wire.
 */
function modeOption(configOptions: SessionConfigOption[] | null | undefined): Extract<
  SessionConfigOption, { type: 'select' }
> {
  return selectOption(configOptions, 'mode')
}

/** The values a client's picker would list, in roster order. */
function modeValues(configOptions: SessionConfigOption[] | null | undefined): string[] {
  return modeOption(configOptions).options.map(value => ('value' in value ? value.value : value.group))
}

describe('agent-preset modes', () => {
  let harness: BridgeHarness | undefined

  afterEach(async () => {
    await harness?.dispose()
    harness = undefined
  })

  it('composes the default preset and offers the roster with session/new', async () => {
    harness = await makeBridgeHarness({ presets: { default: 'alpha' } })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const session = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })

    expect(toolNames(harness, session.sessionId)).toEqual(['alpha'])
    const option = modeOption(session.configOptions)
    expect(option.id).toBe('agent-preset')
    expect(option.category).toBe('mode')
    expect(option.currentValue).toBe('alpha')
    // `broken` is withheld: offering it would advertise a choice every attempt
    // refuses. Nothing says the mode is fixed yet, because it is not.
    expect(option.options).toEqual([
      { value: 'alpha', name: 'Alpha mode', description: 'the fixture default' },
      { value: 'beta', name: 'Beta mode', description: 'the fixture alternative' },
    ])
    expect(option.description).toBeUndefined()
  })

  it('recomposes a blank session, without a model turn', async () => {
    harness = await makeBridgeHarness({ presets: { default: 'alpha' } })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const { sessionId } = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })

    const result = await harness.client.setSessionConfigOption({
      sessionId, configId: 'agent-preset', value: 'beta',
    })

    // The exchange is a control, not a turn: the model is never asked.
    expect(harness.adapter.requests).toHaveLength(0)
    expect(toolNames(harness, sessionId)).toEqual(['beta'])
    expect(selections(harness, sessionId)).toEqual(['beta'])
    // The response carries the whole set, so the picker re-renders from it.
    expect(modeOption(result.configOptions).currentValue).toBe('beta')
    expect(modeValues(result.configOptions)).toEqual(['alpha', 'beta'])
  })

  it('answers a repeat of the session\'s own mode without recomposing', async () => {
    harness = await makeBridgeHarness({ presets: { default: 'alpha' } })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const { sessionId } = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })

    const result = await harness.client.setSessionConfigOption({
      sessionId, configId: 'agent-preset', value: 'alpha',
    })

    expect(modeOption(result.configOptions).currentValue).toBe('alpha')
    expect(selections(harness, sessionId)).toEqual([])
    expect(toolNames(harness, sessionId)).toEqual(['alpha'])
  })

  it('refuses an option, a mode, and a value type it does not have', async () => {
    harness = await makeBridgeHarness({ presets: { default: 'alpha' } })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const { sessionId } = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })

    await expect(harness.client.setSessionConfigOption({
      sessionId, configId: 'thought-level', value: 'high',
    })).rejects.toThrow(/unknown config option: thought-level/)
    await expect(harness.client.setSessionConfigOption({
      sessionId, configId: 'agent-preset', value: 'gamma',
    })).rejects.toThrow(/unknown mode: gamma/)
    await expect(harness.client.setSessionConfigOption({
      sessionId, configId: 'agent-preset', value: 'broken',
    })).rejects.toThrow(/cannot be used/)
    await expect(harness.client.setSessionConfigOption({
      sessionId, configId: 'agent-preset', type: 'boolean', value: true,
    })).rejects.toThrow(/select option, not a boolean/)

    expect(toolNames(harness, sessionId)).toEqual(['alpha'])
    expect(selections(harness, sessionId)).toEqual([])
  })

  it('fixes the mode once the conversation has started, and says so once', async () => {
    harness = await makeBridgeHarness({
      presets: { default: 'alpha' },
      script: [textResponse('hi'), textResponse('again')],
    })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const { sessionId } = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })
    await harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'hello' }] })

    await expect(harness.client.setSessionConfigOption({
      sessionId, configId: 'agent-preset', value: 'beta',
    })).rejects.toThrow(/already started/)

    expect(toolNames(harness, sessionId)).toEqual(['alpha'])
    expect(selections(harness, sessionId)).toEqual([])
    // The picker was shrunk to the mode in force, which is how a client learns
    // to disable it without knowing anything about presets.
    await waitForUpdates(harness, 'config_option_update', 1)
    const [locked, ...repeats] = updatesOfKind(harness, 'config_option_update')
    expect(modeValues(locked?.configOptions)).toEqual(['alpha'])
    expect(modeOption(locked?.configOptions).description).toMatch(/already started/)

    // A second turn does not say it again.
    await harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'more' }] })
    expect(repeats).toEqual([])
    expect(updatesOfKind(harness, 'config_option_update')).toHaveLength(1)
  })

  it('offers no mode when the deployment has no roster', async () => {
    harness = await makeBridgeHarness({ script: [textResponse('ok')] })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const session = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })

    // The model picker is orthogonal and still published; only the mode is gone.
    expect(optionsOfCategory(session.configOptions, 'mode')).toEqual([])
    await expect(harness.client.setSessionConfigOption({
      sessionId: session.sessionId, configId: 'agent-preset', value: 'beta',
    })).rejects.toThrow(/offers no modes/)

    // A turn still runs, and still announces nothing about modes.
    await harness.client.prompt({ sessionId: session.sessionId, prompt: [{ type: 'text', text: 'hi' }] })
    expect(harness.adapter.requests).toHaveLength(1)
    expect(updatesOfKind(harness, 'config_option_update')).toEqual([])
  })
})
