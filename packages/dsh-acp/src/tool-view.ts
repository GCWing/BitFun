/**
 * Presentation mapping from harness tool vocabulary to the ACP tool-call shape
 * an IDE renders. This is the part `@deepseek-ai/dsh-acp` deliberately omits:
 * its wire is automation-only, so it publishes no tool identity at all.
 *
 * @module @bitfun/dsh-acp/tool-view
 */

import type { ToolCallContent, ToolCallLocation, ToolKind } from '@agentclientprotocol/sdk'
import type { ContentBlock, ToolResultMessage } from '@deepseek-ai/dsh-llm'

/**
 * Harness tool name to the ACP kind vocabulary. Exact names only: a client's
 * icon and grouping should never hinge on a substring accident, and an
 * unrecognized tool is honestly `other` rather than mis-labelled.
 */
const TOOL_KINDS: Readonly<Record<string, ToolKind>> = {
  ask_user_question: 'other',
  bash: 'execute',
  create_goal: 'other',
  edit: 'edit',
  exit_plan_mode: 'switch_mode',
  get_goal: 'other',
  glob: 'search',
  grep: 'search',
  interrupt_agent: 'other',
  job_kill: 'other',
  job_list: 'other',
  job_output: 'other',
  list_agents: 'other',
  lsp: 'search',
  pwsh: 'execute',
  ralph: 'other',
  read: 'read',
  read_image: 'read',
  report: 'other',
  run_code: 'execute',
  send_message: 'other',
  session_event_read: 'search',
  session_event_search: 'search',
  session_event_trace: 'search',
  session_search: 'search',
  session_trace: 'search',
  skill: 'other',
  str_replace_editor: 'edit',
  subagent: 'other',
  subagent_fork: 'other',
  terminal_close: 'execute',
  terminal_list: 'other',
  terminal_open: 'execute',
  terminal_read: 'read',
  terminal_send: 'execute',
  terminal_signal: 'execute',
  todo_write: 'other',
  update_goal: 'other',
  web_fetch: 'fetch',
  web_search: 'search',
  workflow: 'other',
  write: 'edit',
}

/**
 * Classify a harness tool for the client's tool card.
 * @param name - the harness tool name as registered with `ctx.tools`.
 * @returns the ACP kind, defaulting to `other` for tools this bridge has not seen.
 */
export function toolKind(name: string): ToolKind {
  return TOOL_KINDS[name] ?? 'other'
}

/**
 * Decode the model's raw argument string. The harness carries arguments as the
 * provider emitted them, so a truncated or malformed call must still produce a
 * card: an unparseable payload is surfaced as the literal text instead of
 * failing the update.
 * @param args - the raw JSON string from `tool/call`.
 * @returns the parsed object, or the original string when it is not JSON.
 */
export function parseToolArguments(args: string): unknown {
  if (args.length === 0) return {}
  try {
    return JSON.parse(args)
  } catch {
    return args
  }
}

/** Argument keys, in priority order, that name the file a tool acts on. */
const PATH_KEYS = ['file_path', 'path', 'notebook_path', 'filePath']

/**
 * Extract the file locations a tool call touches so the client can offer
 * "follow along" navigation.
 * @param input - the parsed tool arguments.
 * @returns one location when the arguments name a path, otherwise none.
 */
export function toolLocations(input: unknown): ToolCallLocation[] {
  if (typeof input !== 'object' || input === null) return []
  const record = input as Record<string, unknown>
  for (const key of PATH_KEYS) {
    const value = record[key]
    if (typeof value !== 'string' || value.length === 0) continue
    const line = record['offset'] ?? record['line']
    return [{ path: value, ...typeof line === 'number' ? { line } : {} }]
  }
  return []
}

/** The harness tool-result payload a client can render, lifted out of the message. */
export interface ToolOutcome {
  /** The call this result settles, taken from the result block or the message source. */
  callId: string | undefined
  /** Renderable result blocks in wire order. */
  content: ToolCallContent[]
  /** Whether the tool reported failure, from the result block or the event's error field. */
  isError: boolean
}

/**
 * Render one harness content block for a tool card.
 * @param block - a block from the tool-result payload.
 * @returns the ACP content entry, or undefined for blocks with no client rendering.
 */
function renderBlock(block: ContentBlock): ToolCallContent | undefined {
  switch (block.type) {
    case 'text':
      return { type: 'content', content: { type: 'text', text: block.text } }
    case 'reasoning':
      return { type: 'content', content: { type: 'text', text: block.text } }
    case 'image':
      return {
        type: 'content',
        content: { type: 'text', text: `[image attachment ${block.attachment.attachmentId}]` },
      }
    default:
      return undefined
  }
}

/**
 * Lift the renderable outcome out of a harness `tool/result` payload.
 * @param message - the tool-result message the harness appended.
 * @param hasError - whether the event carried a structured error alongside the message.
 * @returns the call correlation, rendered content, and failure flag.
 */
export function toolOutcome(message: ToolResultMessage, hasError: boolean): ToolOutcome {
  const block = message.content.find(entry => entry.type === 'tool-result')
  const content = block === undefined
    ? []
    : block.content.flatMap(entry => {
      const rendered = renderBlock(entry)
      return rendered === undefined ? [] : [rendered]
    })
  return {
    callId: block?.toolCallId ?? message.source.callId,
    content,
    isError: hasError || block?.isError === true,
  }
}
