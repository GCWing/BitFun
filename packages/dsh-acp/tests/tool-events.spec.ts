/** The presentation data this fork exists to publish: tool cards, plans, text. */

import { afterEach, describe, expect, it } from 'vitest'
import { PROTOCOL_VERSION } from '@agentclientprotocol/sdk'
import { CallId, createAssistantMessage, createToolResultMessage } from '@deepseek-ai/dsh-llm'
import type { Agent } from '@deepseek-ai/dsh-agent'
import { SessionId } from '@deepseek-ai/dsh-session'
import { makeBridgeHarness, updatesOfKind, waitForEvents, type BridgeHarness } from './harness.ts'

/** Surface-eligible events must declare how they enter the ordered surface. */
const SURFACE = { surfaceOp: 'append' } as const

/** The provenance every scripted assistant message carries in these tests. */
const PROVENANCE = { provider: 'mock', model: 'mock' } as const

describe('ACP session-event publication', () => {
  let harness: BridgeHarness | undefined

  afterEach(async () => {
    await harness?.dispose()
    harness = undefined
  })

  /** Open a session and hand back its live agent, ready to append events to. */
  async function openSession(config?: { reasoning?: boolean }): Promise<Agent> {
    harness = await makeBridgeHarness(config === undefined ? {} : { config })
    await harness.client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
    const { sessionId } = await harness.client.newSession({ cwd: process.cwd(), mcpServers: [] })
    const agent = harness.ctx.agents.get(SessionId(sessionId))
    if (agent === undefined) throw new Error('missing agent')
    agent.session.append('turn/start', { turn: 1 })
    agent.session.append('step/start', { turn: 1, step: 0 })
    return agent
  }

  it('publishes a tool call as a pending card and its result as completed', async () => {
    const agent = await openSession()
    const callId = CallId('call-1')
    agent.session.append('tool/call', {
      turn: 1,
      step: 0,
      callId,
      name: 'read',
      arguments: JSON.stringify({ file_path: '/repo/src/main.ts', offset: 40 }),
    })
    agent.session.append('tool/result', {
      turn: 1,
      step: 0,
      message: createToolResultMessage({
        callId, content: [{ type: 'text', text: 'file body' }], isError: false,
      }),
    }, SURFACE)
    await waitForEvents(harness!, 2)

    expect(updatesOfKind(harness!, 'tool_call')[0]).toMatchObject({
      sessionUpdate: 'tool_call',
      toolCallId: 'call-1',
      title: 'read',
      kind: 'read',
      status: 'pending',
      rawInput: { file_path: '/repo/src/main.ts', offset: 40 },
      locations: [{ path: '/repo/src/main.ts', line: 40 }],
    })
    expect(updatesOfKind(harness!, 'tool_call_update')[0]).toMatchObject({
      sessionUpdate: 'tool_call_update',
      toolCallId: 'call-1',
      status: 'completed',
      content: [{ type: 'content', content: { type: 'text', text: 'file body' } }],
    })
  })

  it('maps each harness tool name to its ACP kind', async () => {
    const agent = await openSession()
    const cases = [
      { name: 'bash', kind: 'execute' },
      { name: 'edit', kind: 'edit' },
      { name: 'grep', kind: 'search' },
      { name: 'web_fetch', kind: 'fetch' },
      { name: 'todo_write', kind: 'other' },
      { name: 'a_tool_nobody_registered', kind: 'other' },
    ]
    cases.forEach((entry, index) => {
      agent.session.append('tool/call', {
        turn: 1, step: 0, callId: CallId(`call-${index}`), name: entry.name, arguments: '{}',
      })
    })
    await waitForEvents(harness!, cases.length)

    expect(updatesOfKind(harness!, 'tool_call').map(update => ({
      name: update.title, kind: update.kind,
    }))).toEqual(cases)
  })

  it('reports a failed tool call as failed and carries the raw result', async () => {
    const agent = await openSession()
    const callId = CallId('call-boom')
    agent.session.append('tool/call', {
      turn: 1, step: 0, callId, name: 'bash', arguments: JSON.stringify({ command: 'false' }),
    })
    agent.session.append('tool/result', {
      turn: 1,
      step: 0,
      message: createToolResultMessage({
        callId, content: [{ type: 'text', text: 'exit status 1' }], isError: true,
      }),
      error: { name: 'ToolError', code: 'EXIT_STATUS' },
    }, SURFACE)
    await waitForEvents(harness!, 2)

    const settled = updatesOfKind(harness!, 'tool_call_update')[0]
    expect(settled).toMatchObject({ toolCallId: 'call-boom', status: 'failed' })
    expect(settled?.rawOutput).toMatchObject({ content: [{ type: 'tool-result', toolCallId: 'call-boom' }] })
  })

  it('keeps a tool call whose arguments are not JSON', async () => {
    const agent = await openSession()
    agent.session.append('tool/call', {
      turn: 1, step: 0, callId: CallId('call-partial'), name: 'bash', arguments: '{"command": "ec',
    })
    await waitForEvents(harness!, 1)

    expect(updatesOfKind(harness!, 'tool_call')[0]).toMatchObject({
      toolCallId: 'call-partial', kind: 'execute', rawInput: '{"command": "ec', locations: [],
    })
  })

  it('streams deltas and does not repeat them when the message commits', async () => {
    const agent = await openSession()
    agent.session.append('assistant/chunk', {
      turn: 1, step: 0, chunk: { type: 'reasoning-delta', index: 0, text: 'thinking' },
    })
    agent.session.append('assistant/chunk', {
      turn: 1, step: 0, chunk: { type: 'text-delta', index: 0, text: 'hel' },
    })
    agent.session.append('assistant/chunk', {
      turn: 1, step: 0, chunk: { type: 'text-delta', index: 0, text: 'lo' },
    })
    agent.session.append('assistant/message', {
      turn: 1,
      step: 0,
      message: createAssistantMessage({
        source: PROVENANCE,
        content: [{ type: 'reasoning', text: 'thinking' }, { type: 'text', text: 'hello' }],
      }),
    }, SURFACE)
    await waitForEvents(harness!, 3)

    expect(updatesOfKind(harness!, 'agent_message_chunk').map(update => update.content))
      .toEqual([{ type: 'text', text: 'hel' }, { type: 'text', text: 'lo' }])
    expect(updatesOfKind(harness!, 'agent_thought_chunk').map(update => update.content))
      .toEqual([{ type: 'text', text: 'thinking' }])
  })

  it('publishes a committed message that never streamed', async () => {
    const agent = await openSession()
    agent.session.append('assistant/message', {
      turn: 1,
      step: 0,
      message: createAssistantMessage({ source: PROVENANCE, content: [{ type: 'text', text: 'whole answer' }] }),
    }, SURFACE)
    await waitForEvents(harness!, 1)

    expect(updatesOfKind(harness!, 'agent_message_chunk').map(update => update.content))
      .toEqual([{ type: 'text', text: 'whole answer' }])
  })

  it('re-publishes committed text for a later step of the same turn', async () => {
    const agent = await openSession()
    agent.session.append('assistant/chunk', {
      turn: 1, step: 0, chunk: { type: 'text-delta', index: 0, text: 'streamed' },
    })
    agent.session.append('step/end', { turn: 1, step: 0 })
    agent.session.append('step/start', { turn: 1, step: 1 })
    agent.session.append('assistant/message', {
      turn: 1,
      step: 1,
      message: createAssistantMessage({ source: PROVENANCE, content: [{ type: 'text', text: 'second step' }] }),
    }, SURFACE)
    await waitForEvents(harness!, 2)

    expect(updatesOfKind(harness!, 'agent_message_chunk').map(update => update.content))
      .toEqual([{ type: 'text', text: 'streamed' }, { type: 'text', text: 'second step' }])
  })

  it('withholds reasoning when the deployment turns it off', async () => {
    const agent = await openSession({ reasoning: false })
    agent.session.append('assistant/chunk', {
      turn: 1, step: 0, chunk: { type: 'reasoning-delta', index: 0, text: 'private' },
    })
    agent.session.append('assistant/chunk', {
      turn: 1, step: 0, chunk: { type: 'text-delta', index: 0, text: 'public' },
    })
    agent.session.append('assistant/message', {
      turn: 1,
      step: 0,
      message: createAssistantMessage({
        source: PROVENANCE,
        content: [{ type: 'reasoning', text: 'private' }, { type: 'text', text: 'public' }],
      }),
    }, SURFACE)
    await waitForEvents(harness!, 1)

    expect(updatesOfKind(harness!, 'agent_thought_chunk')).toHaveLength(0)
    expect(updatesOfKind(harness!, 'agent_message_chunk').map(update => update.content))
      .toEqual([{ type: 'text', text: 'public' }])
  })

  it('republishes each todo write as a plan', async () => {
    const agent = await openSession()
    agent.session.append('todo/write', {
      todos: [
        { content: 'read the failing test', status: 'completed' },
        { content: 'fix the parser', status: 'in_progress' },
        { content: 'run the suite', status: 'pending' },
      ],
    })
    await waitForEvents(harness!, 1)

    expect(updatesOfKind(harness!, 'plan')[0]).toEqual({
      sessionUpdate: 'plan',
      entries: [
        { content: 'read the failing test', priority: 'medium', status: 'completed' },
        { content: 'fix the parser', priority: 'medium', status: 'in_progress' },
        { content: 'run the suite', priority: 'medium', status: 'pending' },
      ],
    })
  })

  it('keeps harness bookkeeping events off the wire', async () => {
    const agent = await openSession()
    agent.session.append('step/end', { turn: 1, step: 0 })
    agent.session.append('turn/end', { turn: 1, reason: { kind: 'completed' } })
    agent.session.append('turn/start', { turn: 2 })
    agent.session.append('step/start', { turn: 2, step: 0 })
    agent.session.append('assistant/chunk', {
      turn: 2, step: 0, chunk: { type: 'text-delta', index: 0, text: 'next turn' },
    })
    await waitForEvents(harness!, 1)

    expect(harness!.updates.map(update => update.sessionUpdate)).toEqual(['agent_message_chunk'])
  })
})
