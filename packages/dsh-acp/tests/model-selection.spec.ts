/**
 * The model picker: the harness's model catalog offered as a `model` session
 * config option, and the pick routed to the next model call.
 *
 * ACP 0.25 has no model state of its own, so a client's model dropdown IS a
 * config option — a session that publishes none leaves the IDE with nothing to
 * pick from, which is the failure these tests exist to keep out. The switch is
 * driven the way a composer drives it, through `session/set_config_option`, and
 * read back off the request the scripted adapter actually received rather than
 * off anything the bridge reports about itself.
 */

import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { randomUUID } from 'node:crypto'
import { afterEach, describe, expect, it } from 'vitest'
import { PROTOCOL_VERSION } from '@agentclientprotocol/sdk'
import { createUserMessage } from '@deepseek-ai/dsh-llm'
import { SessionId } from '@deepseek-ai/dsh-session'
import {
  makeBridgeHarness, optionsOfCategory, selectOption, textResponse, updatesOfKind, waitForUpdates,
  type BridgeHarness, type CatalogProvider,
} from './harness.ts'

/** Two providers, so grouping and cross-provider switching are both observable. */
const CATALOG: Record<string, CatalogProvider> = {
  mock: {
    name: 'Mock',
    models: [
      { id: 'mock', name: 'Mock' },
      { id: 'mock-pro', name: 'Mock Pro', description: 'the bigger one' },
    ],
  },
  alt: { name: 'Alt Cloud', models: [{ id: 'alt-1', name: 'Alt One' }] },
}

/** The provider/model each recorded model call was routed to, in order. */
function routed(harness: BridgeHarness): string[] {
  return harness.adapter.requests.map(request => `${request.provider}/${request.model}`)
}

describe('model selection', () => {
  let harness: BridgeHarness | undefined
  let root: string | undefined

  afterEach(async () => {
    await harness?.dispose()
    harness = undefined
    if (root !== undefined) await rm(root, { recursive: true, force: true })
    root = undefined
  })

  it('offers the catalog grouped by provider, on the model the session runs', async () => {
    harness = await makeBridgeHarness({ catalog: CATALOG })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const session = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })

    const option = selectOption(session.configOptions, 'model')
    expect(option.id).toBe('model')
    expect(option.currentValue).toBe('mock/mock')
    // Grouped by provider and keyed by the pair, because the same model id can
    // be served by two routes and a bare id would not say which one ran.
    expect(option.options).toEqual([
      {
        group: 'mock',
        name: 'Mock',
        options: [
          { value: 'mock/mock', name: 'Mock' },
          { value: 'mock/mock-pro', name: 'Mock Pro', description: 'the bigger one' },
        ],
      },
      { group: 'alt', name: 'Alt Cloud', options: [{ value: 'alt/alt-1', name: 'Alt One' }] },
    ])
  })

  it('lists a model shared by two routes once, under the route in force', async () => {
    // Two provider ids serving the same catalog is a real deployment, not a
    // mistake: a second route to the same vendor, mounted under its own id so
    // it can carry its own key. Listed per provider, every model appeared
    // twice, with only the value prefix telling the rows apart.
    harness = await makeBridgeHarness({
      catalog: {
        mock: {
          name: 'Mock',
          models: [
            { id: 'mock', name: 'Mock' },
            { id: 'mock-pro', name: 'Mock Pro', description: 'the bigger one' },
          ],
        },
        mirror: {
          name: 'Mirror',
          models: [
            { id: 'mock', name: 'Mock' },
            { id: 'mock-pro', name: 'Mock Pro', description: 'the bigger one' },
            { id: 'mirror-only', name: 'Mirror Only' },
          ],
        },
      },
      config: { provider: 'mirror', model: 'mock' },
    })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const session = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })

    const option = selectOption(session.configOptions, 'model')
    // The session runs on `mirror`, so `mirror` owns every id it shares — the
    // checked row keeps the route that actually answers, and `mock` is left
    // with nothing of its own to list.
    expect(option.currentValue).toBe('mirror/mock')
    expect(option.options).toEqual([
      {
        group: 'mirror',
        name: 'Mirror',
        options: [
          { value: 'mirror/mock', name: 'Mock' },
          { value: 'mirror/mock-pro', name: 'Mock Pro', description: 'the bigger one' },
          { value: 'mirror/mirror-only', name: 'Mirror Only' },
        ],
      },
    ])
  })

  it('routes the next turn to the model the client picked', async () => {
    harness = await makeBridgeHarness({
      catalog: CATALOG,
      script: [textResponse('before'), textResponse('after')],
    })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const { sessionId } = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })
    await harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'hello' }] })

    const result = await harness.client.setSessionConfigOption({
      sessionId, configId: 'model', value: 'alt/alt-1',
    })

    // Picking a model is a control, not a turn.
    expect(routed(harness)).toEqual(['mock/mock'])
    expect(selectOption(result.configOptions, 'model').currentValue).toBe('alt/alt-1')

    await harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'again' }] })
    expect(routed(harness)).toEqual(['mock/mock', 'alt/alt-1'])
  })

  it('stays switchable after the conversation has started, unlike the mode', async () => {
    harness = await makeBridgeHarness({
      catalog: CATALOG,
      presets: { default: 'alpha' },
      script: [textResponse('hi')],
    })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const { sessionId } = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })
    await harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'hello' }] })

    // The mode is fixed by now; the model is not, because swapping which model
    // answers the next step leaves every logged turn valid.
    await expect(harness.client.setSessionConfigOption({
      sessionId, configId: 'agent-preset', value: 'beta',
    })).rejects.toThrow(/already started/)
    const result = await harness.client.setSessionConfigOption({
      sessionId, configId: 'model', value: 'mock/mock-pro',
    })

    expect(selectOption(result.configOptions, 'model').currentValue).toBe('mock/mock-pro')
    // The mode rides along, still locked: every publication replaces the whole
    // set, so an answer carrying only the model would drop the mode picker.
    expect(selectOption(result.configOptions, 'mode').description).toMatch(/already started/)
  })

  it('keeps the picker in the update that locks the mode', async () => {
    harness = await makeBridgeHarness({
      catalog: CATALOG, presets: { default: 'alpha' }, script: [textResponse('hi')],
    })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const { sessionId } = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })
    await harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'hello' }] })

    await waitForUpdates(harness, 'config_option_update', 1)
    const [locked] = updatesOfKind(harness, 'config_option_update')
    expect(selectOption(locked?.configOptions, 'model').currentValue).toBe('mock/mock')
  })

  it('refuses a model the harness cannot serve', async () => {
    harness = await makeBridgeHarness({ catalog: CATALOG })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const { sessionId } = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })

    await expect(harness.client.setSessionConfigOption({
      sessionId, configId: 'model', value: 'ghost/anything',
    })).rejects.toThrow(/cannot use model "ghost\/anything"/)
    // A value with no provider half names nothing: the pair is the identity.
    await expect(harness.client.setSessionConfigOption({
      sessionId, configId: 'model', value: 'alt-1',
    })).rejects.toThrow(/unknown model: alt-1/)
    await expect(harness.client.setSessionConfigOption({
      sessionId, configId: 'model', type: 'boolean', value: true,
    })).rejects.toThrow(/select option, not a boolean/)

    const session = await harness.client.loadSession({ sessionId, cwd: process.cwd(), mcpServers: [] })
    expect(selectOption(session.configOptions, 'model').currentValue).toBe('mock/mock')
  })

  it('offers the model in force even when the catalog does not list it', async () => {
    // The catalog is advisory — an adapter may accept ids it never advertised —
    // and a picker whose current value is missing from its own options renders
    // blank, which is the same broken dropdown as having no options at all.
    harness = await makeBridgeHarness({ catalog: CATALOG, config: { model: 'unlisted' } })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const session = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })

    const option = selectOption(session.configOptions, 'model')
    expect(option.currentValue).toBe('mock/unlisted')
    expect(option.options[0]).toEqual({
      group: 'mock',
      name: 'Mock',
      options: [
        { value: 'mock/mock', name: 'Mock' },
        { value: 'mock/mock-pro', name: 'Mock Pro', description: 'the bigger one' },
        { value: 'mock/unlisted', name: 'unlisted' },
      ],
    })
  })

  it('reopens a cold session on the model its turns ran under', async () => {
    root = await mkdtemp(join(tmpdir(), 'dsh-acp-model-'))
    harness = await makeBridgeHarness({
      catalog: CATALOG, persistenceRoot: root, script: [textResponse('stored answer')],
    })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const cwd = process.cwd()
    // Seeded outside the bridge and left to go cold: nothing about the session
    // is in memory when the load arrives, so the picker has only the log.
    const sessionId = SessionId(randomUUID())
    const handle = await harness.ctx.agents.create({
      sessionId, meta: { cwd }, agentOptions: { provider: 'alt', model: 'alt-1' },
    })
    handle.agent.followup(createUserMessage({
      content: [{ type: 'text', text: 'stored question' }], source: { kind: 'user' },
    }))
    await handle.agent.whenIdle()
    await handle.dispose()

    const loaded = await harness.client.loadSession({ sessionId, cwd, mcpServers: [] })

    // `mock/mock` is what a fresh session would open on; the logged header wins.
    expect(selectOption(loaded.configOptions, 'model').currentValue).toBe('alt/alt-1')
  })

  it('offers no model when nothing pins one', async () => {
    harness = await makeBridgeHarness({
      catalog: CATALOG, config: { provider: undefined, model: undefined },
    })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const session = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })

    // Naming a current value the session does not actually run on would be a
    // lie the client renders as fact, so the picker is withheld instead.
    expect(optionsOfCategory(session.configOptions, 'model')).toEqual([])
  })
})
