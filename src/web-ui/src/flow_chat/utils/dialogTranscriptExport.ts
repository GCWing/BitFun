/**
 * Shared dialog transcript formatting for clipboard copy and Markdown export.
 *
 * Two scopes are supported everywhere a transcript leaves the app:
 * - `full`   — user message, assistant text, thinking, and tool calls.
 * - `result` — user message and assistant text only.
 *
 * The runtime store (`DialogTurn`) and the persisted shape (`DialogTurnData`)
 * are normalized into the same intermediate form so a single formatter serves
 * one-turn copy and whole-session export.
 */

import type {
  AnyFlowItem,
  DialogTurn,
  FlowTextItem,
  FlowThinkingItem,
  FlowToolItem,
  ModelRound,
} from '../types/flow-chat';
import type { DialogTurnData, ModelRoundData } from '@/shared/types/session-history';
import { formatSessionViewPreviewText } from './sessionViewPreview';
import { effectiveToolInvocation } from './toolInvocationIdentity';
import { resolvePersistedUserMessageText } from './userInputText';

/** What a transcript export includes. */
export type TranscriptExportScope = 'full' | 'result';

export interface TranscriptExportItem {
  kind: 'text' | 'thinking' | 'tool';
  /** Text / thinking content. */
  content?: string;
  toolName?: string;
  toolInput?: unknown;
  toolResult?: unknown;
  toolError?: string;
}

export interface TranscriptExportRound {
  items: TranscriptExportItem[];
}

export interface TranscriptExportTurn {
  turnIndex?: number;
  userContent: string;
  rounds: TranscriptExportRound[];
}

/** Localized labels used by the formatters. */
export interface TranscriptExportLabels {
  /** Inline user prefix for clipboard copy, e.g. "👤 User:". */
  userLabel: string;
  /** Inline tool prefix for clipboard copy, e.g. "🔧 Tool call: Read". */
  toolCallLabel: (toolName: string) => string;
  unknownTool: string;
  thinking: string;
  toolInput: string;
  toolResult: string;
  toolError: string;
  empty: string;
}

export interface SessionTranscriptMarkdownLabels extends TranscriptExportLabels {
  scopeFull: string;
  scopeResult: string;
  metaSessionId: string;
  metaExportedAt: string;
  metaTurnCount: string;
  metaScope: string;
  metaWorkspace: string;
  turnHeading: (index: number) => string;
  userHeading: string;
  assistantHeading: string;
  toolHeading: (toolName: string) => string;
}

export interface SessionTranscriptMarkdownMeta {
  title: string;
  sessionId: string;
  exportedAt: Date;
  workspacePath?: string;
}

function normalizeText(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

function stringifyPayload(value: unknown): string {
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value, null, 2) ?? '';
  } catch {
    return String(value);
  }
}

/**
 * Retries append superseded items to the same persisted round. Keep only the
 * items belonging to the highest attempt, matching how the round is rendered.
 */
function effectiveAttemptIndex(round: ModelRoundData): number | undefined {
  const indices = [
    ...round.textItems,
    ...round.toolItems,
    ...(round.thinkingItems ?? []),
  ]
    .map(item => item.attemptIndex)
    .filter((index): index is number => typeof index === 'number');

  return indices.length > 0 ? Math.max(...indices) : undefined;
}

function isEffectiveAttempt(
  attemptIndex: number | undefined,
  effectiveIndex: number | undefined
): boolean {
  if (effectiveIndex === undefined) return true;
  return attemptIndex === effectiveIndex;
}

function toolItemToExportItem(
  toolName: string,
  input: unknown,
  toolResult: { result?: unknown; error?: string } | undefined
): TranscriptExportItem {
  const effective = effectiveToolInvocation(toolName, input);
  return {
    kind: 'tool',
    toolName: effective.toolName || toolName,
    toolInput: effective.input,
    toolResult: toolResult?.error ? undefined : toolResult?.result,
    toolError: toolResult?.error,
  };
}

function collectRuntimeRound(round: ModelRound): TranscriptExportRound {
  const items: TranscriptExportItem[] = [];

  round.items.forEach((item: AnyFlowItem) => {
    if (item.type === 'text') {
      const textItem = item as FlowTextItem;
      const content = normalizeText(textItem.content);
      // Transient runtime status lines are presentation-only.
      if (content && !textItem.runtimeStatus) {
        items.push({ kind: 'text', content });
      }
      return;
    }
    if (item.type === 'thinking') {
      const content = normalizeText((item as FlowThinkingItem).content);
      if (content) {
        items.push({ kind: 'thinking', content });
      }
      return;
    }
    if (item.type === 'tool') {
      const toolItem = item as FlowToolItem;
      if (toolItem.toolCall) {
        items.push(
          toolItemToExportItem(toolItem.toolName, toolItem.toolCall.input, toolItem.toolResult)
        );
      }
    }
  });

  return { items };
}

function collectPersistedRound(round: ModelRoundData): TranscriptExportRound {
  const effectiveIndex = effectiveAttemptIndex(round);

  const ordered = [
    ...round.textItems.map(item => ({
      order: item.orderIndex ?? item.timestamp ?? 0,
      attemptIndex: item.attemptIndex,
      build: (): TranscriptExportItem | null => {
        const content = normalizeText(item.content);
        return content ? { kind: 'text', content } : null;
      },
    })),
    ...(round.thinkingItems ?? []).map(item => ({
      order: item.orderIndex ?? item.timestamp ?? 0,
      attemptIndex: item.attemptIndex,
      build: (): TranscriptExportItem | null => {
        const content = normalizeText(item.content);
        return content ? { kind: 'thinking', content } : null;
      },
    })),
    ...round.toolItems.map(item => ({
      order: item.orderIndex ?? item.startTime ?? 0,
      attemptIndex: item.attemptIndex,
      build: (): TranscriptExportItem | null =>
        toolItemToExportItem(item.toolName, item.toolCall?.input, item.toolResult),
    })),
  ]
    .filter(entry => isEffectiveAttempt(entry.attemptIndex, effectiveIndex))
    .sort((a, b) => a.order - b.order);

  const items = ordered
    .map(entry => entry.build())
    .filter((item): item is TranscriptExportItem => item !== null);

  return { items };
}

/** Normalize a live/hydrated dialog turn from the FlowChat store. */
export function collectRuntimeTurn(turn: DialogTurn): TranscriptExportTurn {
  return {
    userContent: normalizeText(turn.userMessage?.content),
    rounds: (turn.modelRounds ?? []).map(collectRuntimeRound),
  };
}

/** Normalize a persisted dialog turn loaded from session storage. */
export function collectPersistedTurn(turn: DialogTurnData): TranscriptExportTurn {
  return {
    turnIndex: turn.turnIndex,
    userContent: normalizeText(
      resolvePersistedUserMessageText(turn.userMessage?.content ?? '', turn.userMessage?.metadata)
    ),
    rounds: (turn.modelRounds ?? []).map(collectPersistedRound),
  };
}

function scopedItems(
  round: TranscriptExportRound,
  scope: TranscriptExportScope
): TranscriptExportItem[] {
  if (scope === 'full') return round.items;
  return round.items.filter(item => item.kind === 'text');
}

/** Whether the turn produces any content under the requested scope. */
export function hasTranscriptContent(
  turn: TranscriptExportTurn,
  scope: TranscriptExportScope
): boolean {
  if (turn.userContent) return true;
  return turn.rounds.some(round => scopedItems(round, scope).length > 0);
}

function markdownFence(content: string, language = ''): string {
  // Widen the fence when the payload itself contains a triple backtick.
  const longestRun = (content.match(/`{3,}/g) ?? [])
    .reduce((longest, run) => Math.max(longest, run.length), 0);
  const fence = '`'.repeat(Math.max(3, longestRun + 1));
  return `${fence}${language}\n${content}\n${fence}`;
}

function formatToolBlockText(
  item: TranscriptExportItem,
  labels: TranscriptExportLabels
): string {
  const toolName = item.toolName || labels.unknownTool;
  let block = `${labels.toolCallLabel(toolName)}\n`;

  if (item.toolInput !== undefined && item.toolInput !== null) {
    block += `\n[${labels.toolInput}]\n${markdownFence(stringifyPayload(item.toolInput), 'json')}\n`;
  }

  if (item.toolError) {
    block += `\n[${labels.toolError}]\n${item.toolError}\n`;
  } else if (item.toolResult !== undefined) {
    const result = formatSessionViewPreviewText(stringifyPayload(item.toolResult));
    block += `\n[${labels.toolResult}]\n${markdownFence(result)}\n`;
  }

  return block.trim();
}

/**
 * Clipboard text for a single dialog turn.
 *
 * `full` reproduces the historical copy format (thinking + tool calls);
 * `result` keeps only the user message and the assistant's written answer.
 */
export function formatTranscriptTurnText(
  turn: TranscriptExportTurn,
  scope: TranscriptExportScope,
  labels: TranscriptExportLabels
): string {
  const parts: string[] = [];

  if (turn.userContent) {
    parts.push(`${labels.userLabel}\n${turn.userContent}`);
  }

  turn.rounds.forEach(round => {
    const blocks = scopedItems(round, scope).map(item => {
      if (item.kind === 'text') return item.content ?? '';
      if (item.kind === 'thinking') return `[${labels.thinking}]\n${item.content ?? ''}`;
      return formatToolBlockText(item, labels);
    });

    const roundContent = blocks.filter(block => block.trim().length > 0);
    if (roundContent.length > 0) {
      parts.push(roundContent.join('\n\n'));
    }
  });

  return parts.join('\n\n---\n\n');
}

function formatToolBlockMarkdown(
  item: TranscriptExportItem,
  labels: SessionTranscriptMarkdownLabels
): string[] {
  const blocks: string[] = [`#### ${labels.toolHeading(item.toolName || labels.unknownTool)}`];

  if (item.toolInput !== undefined && item.toolInput !== null) {
    blocks.push(`*${labels.toolInput}*`);
    blocks.push(markdownFence(stringifyPayload(item.toolInput), 'json'));
  }

  if (item.toolError) {
    blocks.push(`*${labels.toolError}*`);
    blocks.push(markdownFence(item.toolError));
  } else if (item.toolResult !== undefined) {
    blocks.push(`*${labels.toolResult}*`);
    blocks.push(markdownFence(formatSessionViewPreviewText(stringifyPayload(item.toolResult))));
  }

  return blocks;
}

/**
 * Markdown document for a whole session.
 *
 * Blocks are joined with a single blank line, so verbatim content (code fences,
 * tool payloads) is never reflowed.
 */
export function formatSessionTranscriptMarkdown(
  turns: TranscriptExportTurn[],
  scope: TranscriptExportScope,
  meta: SessionTranscriptMarkdownMeta,
  labels: SessionTranscriptMarkdownLabels
): string {
  const metaLines = [`- ${labels.metaSessionId}: \`${meta.sessionId}\``];
  if (meta.workspacePath) {
    metaLines.push(`- ${labels.metaWorkspace}: \`${meta.workspacePath}\``);
  }
  metaLines.push(`- ${labels.metaExportedAt}: ${meta.exportedAt.toISOString()}`);
  metaLines.push(`- ${labels.metaTurnCount}: ${turns.length}`);
  metaLines.push(
    `- ${labels.metaScope}: ${scope === 'full' ? labels.scopeFull : labels.scopeResult}`
  );

  const blocks: string[] = [`# ${meta.title}`, metaLines.join('\n')];

  turns.forEach((turn, position) => {
    blocks.push('---');
    blocks.push(`## ${labels.turnHeading(turn.turnIndex ?? position + 1)}`);
    blocks.push(`### ${labels.userHeading}`);
    blocks.push(turn.userContent || `*${labels.empty}*`);

    const assistantBlocks: string[] = [];
    turn.rounds.forEach(round => {
      scopedItems(round, scope).forEach(item => {
        if (item.kind === 'text') {
          assistantBlocks.push(item.content ?? '');
          return;
        }
        if (item.kind === 'thinking') {
          assistantBlocks.push(`#### ${labels.thinking}`);
          assistantBlocks.push(markdownFence(item.content ?? ''));
          return;
        }
        assistantBlocks.push(...formatToolBlockMarkdown(item, labels));
      });
    });

    if (assistantBlocks.length > 0) {
      blocks.push(`### ${labels.assistantHeading}`);
      blocks.push(...assistantBlocks);
    }
  });

  return `${blocks.filter(block => block.trim().length > 0).join('\n\n')}\n`;
}
