/**
 * Regression cover for the upstream machinery this fork keeps verbatim: prompt
 * settlement, cancellation, and the single in-flight slot. Where behavior
 * intentionally diverges — a streaming client sees deltas, including a failed
 * turn's partial text — the test says so.
 */

import { afterEach, describe, expect, it, vi } from 'vitest'
import { PROTOCOL_VERSION } from '@agentclientprotocol/sdk'
import { SessionId } from '@deepseek-ai/dsh-session'
import { errorResponse, makeBridgeHarness, textResponse, type BridgeHarness } from './harness.ts'

/** All published assistant text, concatenated in arrival order. */
function messageText(harness: BridgeHarness): string {
  return harness.updates.flatMap(update => (
    update.sessionUpdate === 'agent_message_chunk' && update.content.type === 'text'
      ? [update.content.text]
      : []
  )).join('')
}

describe('ACP prompt lifecycle', () => {
  let harness: BridgeHarness | undefined

  afterEach(async () => {
    await harness?.dispose()
    harness = undefined
  })

  async function newSession(): Promise<string> {
    if (harness === undefined) throw new Error('missing harness')
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    return (await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })).sessionId
  }

  it('streams a whole turn and settles it as end_turn', async () => {
    harness = await makeBridgeHarness({ script: [textResponse('answer')] })
    const sessionId = await newSession()
    await expect(harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'go' }] }))
      .resolves.toEqual({ stopReason: 'end_turn' })
    await vi.waitFor(() => { expect(messageText(harness!)).toBe('answer') })
    // Character deltas, not one committed blob: this is the streaming an IDE
    // renders live, and the commit must not duplicate it.
    expect(harness.updates.filter(update => update.sessionUpdate === 'agent_message_chunk')).toHaveLength(6)
  })

  it('rejects a failed turn but keeps the partial text it already streamed', async () => {
    harness = await makeBridgeHarness({ script: [errorResponse('provider boom')] })
    const sessionId = await newSession()
    await expect(harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'go' }] }))
      .rejects.toThrow(/turn failed: provider boom/)
    // Deliberate divergence from the automation bridge, which withholds every
    // uncommitted chunk: a live client has already shown these characters, and
    // the prompt rejection is what tells it the turn failed.
    expect(messageText(harness)).toBe('partial')
  })

  it('permits one in-flight prompt and continues the session after a cancel', async () => {
    harness = await makeBridgeHarness({ script: ['hang', textResponse('second')] })
    const sessionId = await newSession()
    const first = harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'one' }] })
    await vi.waitFor(() => { expect(harness!.ctx.agents.get(SessionId(sessionId))?.status).toBe('running') })
    await expect(harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'two' }] }))
      .rejects.toThrow(/already in flight/)

    await harness.client.cancel({ sessionId })
    await expect(first).resolves.toEqual({ stopReason: 'cancelled' })
    await expect(harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'again' }] }))
      .resolves.toEqual({ stopReason: 'end_turn' })
    await vi.waitFor(() => { expect(messageText(harness!)).toContain('second') })
  })

  it('refuses a prompt for an unknown session', async () => {
    harness = await makeBridgeHarness()
    await newSession()
    await expect(harness.client.prompt({ sessionId: 'nope', prompt: [{ type: 'text', text: 'go' }] }))
      .rejects.toThrow(/unknown session/)
  })
})
