/**
 * The whole path in one test: a model asks for a tool, the loop appends
 * `tool/call`, the scheduler asks for approval, the tool runs, and the client
 * sees a card that opens before the prompt and closes after the result.
 *
 * The hand-driven specs assert each mapping in isolation; this one is the only
 * place the ORDER is a fact about the production loop rather than about how a
 * test appended events.
 */

import { afterEach, describe, expect, it, vi } from 'vitest'
import { PROTOCOL_VERSION } from '@agentclientprotocol/sdk'
import { CallId, type StreamChunk } from '@deepseek-ai/dsh-llm'
import { defineContentToolFixture } from '@deepseek-ai/dsh-tools'
import ApprovalService from '@deepseek-ai/dsh-user-approval'
import { makeBridgeHarness, textResponse, type BridgeHarness } from './harness.ts'

/** One scripted model step that calls `write` and stops for its result. */
const CALL_WRITE: StreamChunk[] = [
  { type: 'block-start', index: 0, blockType: 'tool-call' },
  {
    type: 'block-end',
    index: 0,
    block: {
      type: 'tool-call',
      id: CallId('call-write'),
      name: 'write',
      arguments: JSON.stringify({ file_path: '/repo/notes.md', content: 'hi' }),
    },
  },
  { type: 'finish', reason: { kind: 'tool-calls' } },
]

describe('ACP tool-call round trip', () => {
  let harness: BridgeHarness | undefined

  afterEach(async () => {
    await harness?.dispose()
    harness = undefined
  })

  it('opens the card, asks, runs, and closes it — in that order', async () => {
    harness = await makeBridgeHarness({ script: [CALL_WRITE, textResponse('written')] })
    await harness.ctx.plugin(ApprovalService)
    harness.ctx.tools.register(defineContentToolFixture({
      name: 'write',
      description: 'write a file',
      parameters: { file_path: { type: 'string' }, content: { type: 'string' } },
      execute: () => Promise.resolve([{ type: 'text', text: 'wrote 2 bytes' }]),
    }))
    // The production ask path: the scheduler waterfalls `tools/pre-execute`
    // before dispatch, and an `ask` decision routes through ctx.approval.
    harness.ctx.on('tools/pre-execute', () => Promise.resolve({ kind: 'ask', reason: 'test policy' }))
    harness.onPermission = () => ({ outcome: { outcome: 'selected', optionId: 'allow-once' } })

    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const { sessionId } = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })
    const result = await harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'write notes' }] })
    expect(result.stopReason).toBe('end_turn')

    await vi.waitFor(() => {
      expect(harness!.orderedEvents.some(event => event.kind === 'update'
        && event.update.sessionUpdate === 'tool_call_update'
        && event.update.status === 'completed')).toBe(true)
    })

    const trace = harness.orderedEvents.flatMap((event) => {
      if (event.kind === 'permission') return [`permission:${event.request.toolCall.toolCallId}`]
      const update = event.update
      if (update.sessionUpdate === 'tool_call') return [`open:${update.toolCallId}:${update.status ?? ''}`]
      if (update.sessionUpdate === 'tool_call_update') return [`${update.status}:${update.toolCallId}`]
      return []
    })
    expect(trace).toEqual([
      'open:call-write:pending',
      'permission:call-write',
      'in_progress:call-write',
      'completed:call-write',
    ])

    const permission = harness.permissionRequests[0]
    expect(permission?.toolCall).toMatchObject({ title: 'write', kind: 'edit' })
    expect(permission?.toolCall.rawInput).toEqual({ file_path: '/repo/notes.md', content: 'hi' })

    const completed = harness.updates.find(update => update.sessionUpdate === 'tool_call_update'
      && update.status === 'completed')
    expect(completed).toMatchObject({
      content: [{ type: 'content', content: { type: 'text', text: 'wrote 2 bytes' } }],
    })
  })

  it('closes the card as failed when the user rejects the call', async () => {
    harness = await makeBridgeHarness({ script: [CALL_WRITE, textResponse('stopped')] })
    await harness.ctx.plugin(ApprovalService)
    harness.ctx.tools.register(defineContentToolFixture({
      name: 'write',
      description: 'write a file',
      parameters: { file_path: { type: 'string' }, content: { type: 'string' } },
      execute: () => Promise.reject(new Error('must not run')),
    }))
    harness.ctx.on('tools/pre-execute', () => Promise.resolve({ kind: 'ask', reason: 'test policy' }))
    harness.onPermission = () => ({ outcome: { outcome: 'selected', optionId: 'reject-once' } })

    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const { sessionId } = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })
    await harness.client.prompt({ sessionId, prompt: [{ type: 'text', text: 'write notes' }] })

    await vi.waitFor(() => {
      expect(harness!.updates.some(update => update.sessionUpdate === 'tool_call_update')).toBe(true)
    })
    const settled = harness.updates.filter(update => update.sessionUpdate === 'tool_call_update')
    // A rejection never reaches `in_progress`; the denial arrives as the result.
    expect(settled.map(update => update.status)).toEqual(['failed'])
    expect(settled[0]).toMatchObject({ toolCallId: 'call-write' })
  })
})
