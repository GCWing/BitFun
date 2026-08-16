/**
 * A permission prompt must never reference a tool call the client has not been
 * shown — that mismatch is what leaves an IDE waiting for a card that never
 * arrives, and then for a turn that never settles.
 */

import { afterEach, describe, expect, it } from 'vitest'
import { PROTOCOL_VERSION } from '@agentclientprotocol/sdk'
import { CallId } from '@deepseek-ai/dsh-llm'
import type { Agent } from '@deepseek-ai/dsh-agent'
import { SessionId } from '@deepseek-ai/dsh-session'
import ApprovalService from '@deepseek-ai/dsh-user-approval'
import { makeBridgeHarness, waitForEvents, type BridgeHarness, type OrderedEvent } from './harness.ts'

/** The tool-call ids each timeline entry refers to, for order assertions. */
function timeline(events: readonly OrderedEvent[]): string[] {
  return events.map((event) => {
    if (event.kind === 'permission') return `permission:${event.request.toolCall.toolCallId}`
    if (event.update.sessionUpdate === 'tool_call') return `tool_call:${event.update.toolCallId}`
    if (event.update.sessionUpdate === 'tool_call_update') {
      return `tool_call_update:${event.update.toolCallId}:${event.update.status ?? ''}`
    }
    return event.update.sessionUpdate
  })
}

describe('ACP approval ordering', () => {
  let harness: BridgeHarness | undefined

  afterEach(async () => {
    await harness?.dispose()
    harness = undefined
  })

  /** Open a session with the approval waterfall mounted. */
  async function openSession(): Promise<Agent> {
    harness = await makeBridgeHarness()
    await harness.ctx.plugin(ApprovalService)
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const { sessionId } = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })
    const agent = harness.ctx.agents.get(SessionId(sessionId))
    if (agent === undefined) throw new Error('missing agent')
    agent.session.append('turn/start', { turn: 1 })
    agent.session.append('step/start', { turn: 1, step: 0 })
    return agent
  }

  it('shows the tool card before asking permission for it', async () => {
    const agent = await openSession()
    const callId = CallId('call-approve')
    // The agent loop appends tool/call before the scheduler prepares the call,
    // so this is the real production order.
    agent.session.append('tool/call', {
      turn: 1, step: 0, callId, name: 'bash', arguments: JSON.stringify({ command: 'rm -rf build' }),
    })
    await waitForEvents(harness!, 1)
    harness!.onPermission = () => ({ outcome: { outcome: 'selected', optionId: 'allow-once' } })

    await expect(harness!.ctx.approval.request({ agent, toolName: 'bash', callId })).resolves.toBe('allowed-once')

    expect(timeline(harness!.orderedEvents)).toEqual([
      'tool_call:call-approve',
      'permission:call-approve',
      'tool_call_update:call-approve:in_progress',
    ])
    expect(harness!.permissionRequests[0]).toMatchObject({
      sessionId: agent.session.id,
      toolCall: { toolCallId: 'call-approve', title: 'bash', kind: 'execute', status: 'pending' },
      options: [
        { optionId: 'allow-once', kind: 'allow_once' },
        { optionId: 'reject-once', kind: 'reject_once' },
      ],
    })
    expect(harness!.permissionRequests[0]?.toolCall.rawInput).toEqual({ command: 'rm -rf build' })
  })

  it('announces a call the client was never told about before asking', async () => {
    const agent = await openSession()
    harness!.onPermission = () => ({ outcome: { outcome: 'selected', optionId: 'reject-once' } })

    await expect(harness!.ctx.approval.request({ agent, toolName: 'write', callId: CallId('call-unseen') }))
      .resolves.toBe('rejected')

    // Rejected calls stay pending: the harness reports no tool/result for them.
    expect(timeline(harness!.orderedEvents)).toEqual(['tool_call:call-unseen', 'permission:call-unseen'])
    expect(harness!.updates[0]).toMatchObject({
      sessionUpdate: 'tool_call', toolCallId: 'call-unseen', title: 'write', kind: 'edit', status: 'pending',
    })
  })

  it('announces the call once across the event and the approval hook', async () => {
    const agent = await openSession()
    const callId = CallId('call-once')
    agent.session.append('tool/call', {
      turn: 1, step: 0, callId, name: 'edit', arguments: JSON.stringify({ file_path: '/repo/a.ts' }),
    })
    await waitForEvents(harness!, 1)
    harness!.onPermission = () => ({ outcome: { outcome: 'selected', optionId: 'allow-once' } })
    await harness!.ctx.approval.request({ agent, toolName: 'edit', callId })

    expect(timeline(harness!.orderedEvents).filter(entry => entry.startsWith('tool_call:')))
      .toEqual(['tool_call:call-once'])
    // The re-announcement must not overwrite the arguments the event carried.
    expect(harness!.permissionRequests[0]?.toolCall.rawInput).toEqual({ file_path: '/repo/a.ts' })
  })

  it('leaves foreign and identity-less requests to the rest of the waterfall', async () => {
    const agent = await openSession()
    await expect(harness!.ctx.approval.request({ agent, toolName: 'bash' })).resolves.toBe('unavailable')
    expect(harness!.permissionRequests).toHaveLength(0)
    expect(harness!.updates).toHaveLength(0)
  })
})
