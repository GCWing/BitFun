import { describe, expect, it, vi } from 'vitest';

const sessions = new Map<string, any>([
  ['session-1', {
    dialogTurns: [
      {
        id: 'turn-1',
        userMessage: { id: 'u1', content: 'question', timestamp: 1 },
        modelRounds: [
          {
            id: 'round-1',
            items: [
              { id: 'k1', type: 'thinking', content: 'reasoning', timestamp: 2 },
              { id: 't1', type: 'tool', toolName: 'Read', toolCall: { input: { path: 'a.ts' }, id: 'c1' }, timestamp: 3 },
              { id: 'x1', type: 'text', content: 'answer', timestamp: 4 },
            ],
          },
        ],
      },
    ],
  }],
]);

vi.mock('../store/FlowChatStore', () => ({
  FlowChatStore: {
    getInstance: () => ({ getState: () => ({ sessions }) }),
  },
}));

const { buildDialogTurnCopyText, findDialogTurnById } = await import('./dialogTurnCopy');

const labels = {
  userLabel: 'USER:',
  toolCallLabel: (name: string) => `TOOL: ${name}`,
  unknownTool: 'unknown',
  thinking: 'Thinking',
  toolInput: 'Input',
  toolResult: 'Result',
  toolError: 'Error',
  empty: '(empty)',
};

describe('buildDialogTurnCopyText', () => {
  it('finds the turn across sessions', () => {
    expect(findDialogTurnById('turn-1')?.id).toBe('turn-1');
    expect(findDialogTurnById('missing')).toBeUndefined();
  });

  it('copies the full process when asked for it', () => {
    const text = buildDialogTurnCopyText('turn-1', 'full', labels);
    expect(text).toContain('reasoning');
    expect(text).toContain('TOOL: Read');
    expect(text).toContain('answer');
  });

  it('copies only the result when asked for it', () => {
    const text = buildDialogTurnCopyText('turn-1', 'result', labels);
    expect(text).toBe('USER:\nquestion\n\n---\n\nanswer');
  });

  it('returns empty text for an unknown turn', () => {
    expect(buildDialogTurnCopyText('missing', 'full', labels)).toBe('');
  });
});
