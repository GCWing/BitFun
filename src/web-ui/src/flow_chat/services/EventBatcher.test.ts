import { afterEach, describe, expect, it } from 'vitest';
import { setIncludeSensitiveDiagnostics } from '@/shared/utils/logger';
import { getBatchedEventsLogPayload, summarizeBatchedEventsForLog, type BatchedEvent } from './EventBatcher';

describe('summarizeBatchedEventsForLog', () => {
  afterEach(() => {
    setIncludeSensitiveDiagnostics(true);
  });

  it('keeps full payloads when sensitive diagnostics are enabled', () => {
    setIncludeSensitiveDiagnostics(true);
    const events: BatchedEvent[] = [
      {
        key: 'subagent:tool:params:session:call:tool',
        payload: {
          toolEvent: {
            event_type: 'ParamsPartial',
            params: '{"file_path":"src/secret.ts","content":"very sensitive content"}',
          },
        },
        strategy: 'accumulate',
        sourceCount: 12,
        timestamp: 1000,
      },
    ];

    const payloadText = JSON.stringify(getBatchedEventsLogPayload(events));

    expect(payloadText).toContain('very sensitive content');
    expect(payloadText).toContain('src/secret.ts');
  });

  it('keeps batch diagnostics without logging full event payloads', () => {
    setIncludeSensitiveDiagnostics(false);
    const events: BatchedEvent[] = [
      {
        key: 'subagent:tool:params:session:call:tool',
        payload: {
          toolEvent: {
            event_type: 'ParamsPartial',
            params: '{"file_path":"src/secret.ts","content":"very sensitive content"}',
          },
        },
        strategy: 'accumulate',
        sourceCount: 12,
        timestamp: 1000,
      },
    ];

    const summary = summarizeBatchedEventsForLog(events);
    const summaryText = JSON.stringify(summary);

    expect(summary.rawEventCount).toBe(12);
    expect(summary.mergedEventCount).toBe(1);
    expect(summary.events[0]).toEqual({
      key: 'subagent:tool:params:session:call:tool',
      strategy: 'accumulate',
      sourceCount: 12,
      timestamp: 1000,
      eventType: 'ParamsPartial',
      toolName: undefined,
      paramsLength: 64,
    });
    expect(summaryText).not.toContain('very sensitive content');
    expect(summaryText).not.toContain('src/secret.ts');
  });
});
