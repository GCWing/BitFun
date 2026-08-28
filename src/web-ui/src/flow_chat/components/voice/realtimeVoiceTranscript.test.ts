import { describe, expect, it } from 'vitest';
import { applyRealtimeAsrSnapshot } from './realtimeVoiceTranscript';

describe('applyRealtimeAsrSnapshot', () => {
  it('replaces cumulative ASR partials instead of repeating them', () => {
    const transcript = ['当前', '当前项目', '当前项目。']
      .reduce((previous, snapshot) => applyRealtimeAsrSnapshot(previous, snapshot), '');

    expect(transcript).toBe('当前项目。');
    expect(transcript).not.toContain('当前当前');
  });

  it('keeps the display bounded for unusually long partials', () => {
    expect(applyRealtimeAsrSnapshot('', '123456', 4)).toBe('3456');
  });
});
