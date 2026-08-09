import { describe, expect, it, vi } from 'vitest';
import type { DialogTurnData } from '@/shared/types/session-history';

vi.mock('@/infrastructure/api/service-api/SessionAPI', () => ({
  sessionAPI: { loadSessionTurns: vi.fn() },
}));

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: { t: (key: string) => key },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: { success: vi.fn(), warning: vi.fn(), error: vi.fn() },
}));

const { buildSessionExportFileName, buildSessionMarkdown } = await import('./sessionMarkdownExport');

function turn(index: number, answer: string): DialogTurnData {
  return {
    turnId: `turn-${index}`,
    turnIndex: index,
    sessionId: 'session-1',
    timestamp: 1,
    userMessage: { id: `u${index}`, content: `question ${index}`, timestamp: 1 },
    modelRounds: [
      {
        id: `round-${index}`,
        turnId: `turn-${index}`,
        roundIndex: 0,
        timestamp: 1,
        textItems: [{ id: `x${index}`, content: answer, isStreaming: false, timestamp: 2, orderIndex: 1 }],
        toolItems: [],
        thinkingItems: [
          { id: `k${index}`, content: 'internal notes', isStreaming: false, isCollapsed: true, timestamp: 1, orderIndex: 0 },
        ],
        startTime: 1,
        status: 'completed',
      },
    ],
    startTime: 1,
    status: 'completed',
  } as unknown as DialogTurnData;
}

describe('buildSessionExportFileName', () => {
  it('strips path separators and appends a timestamp', () => {
    const name = buildSessionExportFileName(
      'Fix a/b: the "thing"',
      'session-1',
      new Date('2026-07-25T10:11:12.000Z')
    );
    expect(name).toBe('Fix a b the thing_2026-07-25_10-11-12.md');
  });

  it('falls back to the session id when the title has no usable characters', () => {
    const name = buildSessionExportFileName('///', 'session-1', new Date('2026-07-25T10:11:12.000Z'));
    expect(name).toBe('session-1_2026-07-25_10-11-12.md');
  });
});

describe('buildSessionMarkdown', () => {
  const meta = {
    title: 'My session',
    sessionId: 'session-1',
    exportedAt: new Date('2026-07-25T10:00:00.000Z'),
  };

  it('renders every turn of the session', () => {
    const markdown = buildSessionMarkdown([turn(1, 'first answer'), turn(2, 'second answer')], 'full', meta);
    expect(markdown).toContain('first answer');
    expect(markdown).toContain('second answer');
    expect(markdown).toContain('internal notes');
  });

  it('drops thinking when exporting results only', () => {
    const markdown = buildSessionMarkdown([turn(1, 'first answer')], 'result', meta);
    expect(markdown).toContain('first answer');
    expect(markdown).not.toContain('internal notes');
  });
});
