import { describe, expect, it } from 'vitest';
import type { DialogTurn } from '../types/flow-chat';
import type { DialogTurnData } from '@/shared/types/session-history';
import {
  collectPersistedTurn,
  collectRuntimeTurn,
  formatSessionTranscriptMarkdown,
  formatTranscriptTurnText,
  hasTranscriptContent,
  type SessionTranscriptMarkdownLabels,
  type TranscriptExportLabels,
} from './dialogTranscriptExport';

const labels: TranscriptExportLabels = {
  userLabel: 'USER:',
  toolCallLabel: (name: string) => `TOOL: ${name}`,
  unknownTool: 'unknown',
  thinking: 'Thinking',
  toolInput: 'Input',
  toolResult: 'Result',
  toolError: 'Error',
  empty: '(empty)',
};

const markdownLabels: SessionTranscriptMarkdownLabels = {
  ...labels,
  scopeFull: 'Full process',
  scopeResult: 'Result only',
  metaSessionId: 'Session ID',
  metaExportedAt: 'Exported at',
  metaTurnCount: 'Turns',
  metaScope: 'Scope',
  metaWorkspace: 'Workspace',
  turnHeading: (index: number) => `Turn ${index}`,
  userHeading: 'User',
  assistantHeading: 'Assistant',
  toolHeading: (name: string) => `Tool ${name}`,
};

function runtimeTurn(): DialogTurn {
  return {
    id: 'turn-1',
    sessionId: 'session-1',
    userMessage: { id: 'u1', content: 'do the thing', timestamp: 1 },
    modelRounds: [
      {
        id: 'round-1',
        index: 0,
        items: [
          { id: 'k1', type: 'thinking', content: 'secret reasoning', isStreaming: false, isCollapsed: true, timestamp: 2, status: 'completed' },
          { id: 't1', type: 'tool', toolName: 'Read', toolCall: { input: { path: 'a.ts' }, id: 'c1' }, toolResult: { result: 'file body', success: true }, timestamp: 3, status: 'completed' },
          { id: 'x1', type: 'text', content: 'here is the answer', isStreaming: false, timestamp: 4, status: 'completed' },
        ],
        isStreaming: false,
        isComplete: true,
        status: 'completed',
        startTime: 1,
      },
    ],
    startTime: 1,
    status: 'completed',
  } as unknown as DialogTurn;
}

function persistedTurn(): DialogTurnData {
  return {
    turnId: 'turn-1',
    turnIndex: 3,
    sessionId: 'session-1',
    timestamp: 1,
    userMessage: {
      id: 'u1',
      content: '<user_query>persisted question</user_query>',
      timestamp: 1,
    },
    modelRounds: [
      {
        id: 'round-1',
        turnId: 'turn-1',
        roundIndex: 0,
        timestamp: 1,
        textItems: [
          { id: 'x0', content: 'superseded answer', isStreaming: false, timestamp: 2, orderIndex: 0, attemptIndex: 0 },
          { id: 'x1', content: 'final answer', isStreaming: false, timestamp: 5, orderIndex: 2, attemptIndex: 1 },
        ],
        toolItems: [
          {
            id: 't1',
            toolName: 'Read',
            toolCall: { input: { path: 'a.ts' }, id: 'c1' },
            toolResult: { result: 'file body', success: true },
            startTime: 3,
            orderIndex: 1,
            attemptIndex: 1,
          },
        ],
        thinkingItems: [
          { id: 'k1', content: 'secret reasoning', isStreaming: false, isCollapsed: true, timestamp: 4, orderIndex: 0, attemptIndex: 1 },
        ],
        startTime: 1,
        status: 'completed',
      },
    ],
    startTime: 1,
    status: 'completed',
  } as unknown as DialogTurnData;
}

describe('formatTranscriptTurnText', () => {
  it('includes thinking and tool calls in full scope', () => {
    const text = formatTranscriptTurnText(collectRuntimeTurn(runtimeTurn()), 'full', labels);

    expect(text).toContain('USER:\ndo the thing');
    expect(text).toContain('[Thinking]\nsecret reasoning');
    expect(text).toContain('TOOL: Read');
    expect(text).toContain('[Input]');
    expect(text).toContain('[Result]');
    expect(text).toContain('here is the answer');
  });

  it('keeps only the user message and assistant text in result scope', () => {
    const text = formatTranscriptTurnText(collectRuntimeTurn(runtimeTurn()), 'result', labels);

    expect(text).toBe('USER:\ndo the thing\n\n---\n\nhere is the answer');
    expect(text).not.toContain('secret reasoning');
    expect(text).not.toContain('Read');
  });

  it('widens fences so tool payloads cannot break out of the code block', () => {
    const turn = runtimeTurn();
    (turn.modelRounds[0].items[1] as any).toolResult = { result: 'before\n```\nafter', success: true };

    const text = formatTranscriptTurnText(collectRuntimeTurn(turn), 'full', labels);
    expect(text).toContain('````\nbefore\n```\nafter\n````');
  });

});

describe('collectPersistedTurn', () => {
  it('unwraps the user query envelope and keeps the effective attempt in order', () => {
    const turn = collectPersistedTurn(persistedTurn());

    expect(turn.turnIndex).toBe(3);
    expect(turn.userContent).toBe('persisted question');
    expect(turn.rounds[0].items.map(item => item.kind)).toEqual(['thinking', 'tool', 'text']);
    expect(turn.rounds[0].items[2].content).toBe('final answer');
  });

  it('reports content presence per scope', () => {
    const turn = collectPersistedTurn(persistedTurn());
    expect(hasTranscriptContent(turn, 'full')).toBe(true);
    expect(hasTranscriptContent(turn, 'result')).toBe(true);
  });
});

describe('formatSessionTranscriptMarkdown', () => {
  const meta = {
    title: 'My session',
    sessionId: 'session-1',
    exportedAt: new Date('2026-07-25T10:00:00.000Z'),
    workspacePath: '/repo',
  };

  it('renders headings, metadata, and the full process', () => {
    const markdown = formatSessionTranscriptMarkdown(
      [collectPersistedTurn(persistedTurn())],
      'full',
      meta,
      markdownLabels
    );

    expect(markdown).toContain('# My session');
    expect(markdown).toContain('- Session ID: `session-1`');
    expect(markdown).toContain('- Workspace: `/repo`');
    expect(markdown).toContain('- Scope: Full process');
    expect(markdown).toContain('## Turn 3');
    expect(markdown).toContain('### User');
    expect(markdown).toContain('persisted question');
    expect(markdown).toContain('#### Thinking');
    expect(markdown).toContain('#### Tool Read');
    expect(markdown).toContain('final answer');
  });

  it('omits thinking and tools in result scope', () => {
    const markdown = formatSessionTranscriptMarkdown(
      [collectPersistedTurn(persistedTurn())],
      'result',
      meta,
      markdownLabels
    );

    expect(markdown).toContain('- Scope: Result only');
    expect(markdown).toContain('final answer');
    expect(markdown).not.toContain('secret reasoning');
    expect(markdown).not.toContain('#### Tool Read');
  });

  it('widens fences so tool payloads cannot break out of the code block', () => {
    const turn = persistedTurn();
    turn.modelRounds[0].toolItems[0].toolResult = {
      result: 'before\n```\nafter',
      success: true,
    } as any;

    const markdown = formatSessionTranscriptMarkdown(
      [collectPersistedTurn(turn)],
      'full',
      meta,
      markdownLabels
    );

    expect(markdown).toContain('````\nbefore\n```\nafter\n````');
  });
});
