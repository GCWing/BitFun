import { describe, expect, it } from 'vitest';

import { terminalReplayHasScreenText } from './terminalReplay';
import type { TerminalReplayEvent } from '../types';

function replayEvent(data: string, cols = 80, rows = 24): TerminalReplayEvent {
  return { cols, rows, data };
}

describe('terminalReplayHasScreenText', () => {
  it('does not treat pure resize replay events as screen text', () => {
    expect(terminalReplayHasScreenText([
      replayEvent(''),
      replayEvent('', 120, 30),
    ])).toBe(false);
  });

  it('does not treat shell metadata-only replay as screen text', () => {
    expect(terminalReplayHasScreenText([
      replayEvent('\x1b]0;PowerShell\x07'),
      replayEvent('\x1b]633;P;IsWindows=True\x07\x1b]633;P;HasRichCommandDetection=True\x07'),
    ])).toBe(false);
  });

  it('detects printable replay content after terminal control sequences are stripped', () => {
    expect(terminalReplayHasScreenText([
      replayEvent('\x1b[?25l\x1b[32mPS C:\\Workspace\\Project> ls\x1b[m\r\n'),
    ])).toBe(true);
  });
});
