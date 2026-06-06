import { describe, expect, it } from 'vitest';
import type { FlowToolItem } from '../types/flow-chat';
import { buildWriteStdinCardModel } from './execProcessToolCardModel';

const messages: Record<string, string> = {
  'toolCards.execProcess.pollSession': 'Poll session #{{id}}',
  'toolCards.execProcess.pollProcess': 'Poll process:',
  'toolCards.execProcess.writeStdin': 'Write stdin:',
  'toolCards.execProcess.pollingOutput': 'Polling output...',
  'toolCards.execProcess.waitingForOutput': 'Waiting for output...',
  'toolCards.execProcess.noOutput': 'No output',
  'toolCards.execProcess.sessionNotFound': 'Session #{{id}} was not found.',
};

function t(key: string, options?: Record<string, unknown>): string {
  const template = messages[key] ?? String(options?.defaultValue ?? key);
  return template.replace(/{{(\w+)}}/g, (_, name) => String(options?.[name] ?? ''));
}

function writeStdinItem(result: unknown): FlowToolItem {
  return {
    id: 'tool-writestdin-1',
    type: 'tool',
    toolName: 'WriteStdin',
    status: 'completed',
    timestamp: Date.now(),
    toolCall: {
      id: 'call-writestdin-1',
      input: {
        session_id: 42,
        chars: '',
      },
    },
    toolResult: {
      success: true,
      result,
    },
  };
}

describe('buildWriteStdinCardModel', () => {
  it('surfaces session-not-found results as a completed notice', () => {
    const model = buildWriteStdinCardModel(writeStdinItem({
      status: 'session_not_found',
      requested_session_id: 42,
      session_id: null,
      output: '',
      message: 'backend message',
    }), t);

    expect(model.resultNoticeText).toBe('Session #42 was not found.');
    expect(model.resultOutput).toBe('');
    expect(model.noOutputText).toBe('No output');
    expect(model.sessionId).toBe(42);
  });
});
