import { describe, expect, it } from 'vitest';
import { collectAssistantText } from './collectAssistantText';
import type { DialogTurn, FlowTextItem, FlowThinkingItem, FlowToolItem, FlowUserSteeringItem } from '../../types/flow-chat';

function makeTextItem(id: string, content: string, timestamp: number): FlowTextItem {
  return { id, type: 'text', content, isStreaming: false, isMarkdown: true, timestamp, status: 'completed' };
}

function makeThinkingItem(id: string, content: string, timestamp: number): FlowThinkingItem {
  return { id, type: 'thinking', content, isStreaming: false, isCollapsed: true, timestamp, status: 'completed' };
}

function makeToolItem(id: string, timestamp: number): FlowToolItem {
  return {
    id,
    type: 'tool',
    toolName: 'search',
    toolCall: { input: { q: 'x' }, id, timeout_seconds: 10 },
    toolResult: { result: 'ok', success: true },
    timestamp,
    status: 'completed',
  };
}

function makeSteeringItem(id: string, content: string, timestamp: number): FlowUserSteeringItem {
  return { id, type: 'user-steering', steeringId: id, content, roundIndex: 0, timestamp, status: 'completed' };
}

function makeTurn(rounds: { items: any[] }[]): Partial<DialogTurn> {
  return {
    modelRounds: rounds.map((r, i) => ({
      id: `round-${i}`,
      index: i,
      items: r.items,
      isStreaming: false,
      isComplete: true,
      status: 'completed',
      startTime: 0,
    })),
  } as Partial<DialogTurn>;
}

describe('collectAssistantText', () => {
  it('all: joins every assistant text item across rounds with blank line', () => {
    const turn = makeTurn([
      { items: [makeTextItem('t1', 'first answer', 1), makeToolItem('tool1', 2), makeThinkingItem('th1', 'think', 3)] },
      { items: [makeTextItem('t2', 'final answer', 4)] },
    ]);
    expect(collectAssistantText(turn, 'all')).toBe('first answer\n\nfinal answer');
  });

  it('final: returns only the last assistant text item', () => {
    const turn = makeTurn([
      { items: [makeTextItem('t1', 'first answer', 1), makeToolItem('tool1', 2)] },
      { items: [makeTextItem('t2', 'final answer', 4)] },
    ]);
    expect(collectAssistantText(turn, 'final')).toBe('final answer');
  });

  it('excludes thinking, tool, and user-steering items', () => {
    const turn = makeTurn([
      { items: [makeThinkingItem('th1', 'secret thoughts', 1), makeToolItem('tool1', 2), makeSteeringItem('s1', 'steer', 3), makeTextItem('t1', 'only text', 4)] },
    ]);
    expect(collectAssistantText(turn, 'all')).toBe('only text');
  });

  it('ignores empty/whitespace-only text items', () => {
    const turn = makeTurn([
      { items: [makeTextItem('t1', '   ', 1), makeTextItem('t2', '', 2), makeTextItem('t3', 'real', 3)] },
    ]);
    expect(collectAssistantText(turn, 'all')).toBe('real');
    expect(collectAssistantText(turn, 'final')).toBe('real');
  });

  it('returns empty string when no text items exist', () => {
    const turn = makeTurn([{ items: [makeToolItem('tool1', 1), makeThinkingItem('th1', 'think', 2)] }]);
    expect(collectAssistantText(turn, 'all')).toBe('');
    expect(collectAssistantText(turn, 'final')).toBe('');
  });

  it('defaults to all mode', () => {
    const turn = makeTurn([{ items: [makeTextItem('t1', 'a', 1), makeTextItem('t2', 'b', 2)] }]);
    expect(collectAssistantText(turn)).toBe('a\n\nb');
  });
});
