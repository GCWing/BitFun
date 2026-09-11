// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { BackgroundTerminalReplay, sourceTerminalSize } from './backgroundTerminalReplay';
import type { BackgroundCommandOutputMetadata, ReadBackgroundCommandOutputResponse } from '@/infrastructure/api/service-api/AgentAPI';
import recording from './fixtures/conpty-output.json';
import { Terminal as DisplayTerminal } from '@xterm/xterm';

const metadata: BackgroundCommandOutputMetadata = {
  command: 'test', remote: false, tty: true, terminalSize: { cols: 80, rows: 24 },
  status: 'running', startedAt: 0, retainedBytes: 0, retainedLimitBytes: 1024 * 1024,
  truncatedFromStart: false,
};
function response(text: string, cursor = 1, snapshot = true): ReadBackgroundCommandOutputResponse {
  return { metadata, cursor, reset: false, snapshot: snapshot ? text : undefined,
    chunks: snapshot ? [] : [text] };
}

describe('background terminal replay', () => {
  it('restores the recorded ConPTY edge overwrite without duplicate a/R or phantom newlines', async () => {
    const replay = new BackgroundTerminalReplay();
    const phase1 = recording.slice(0, recording.indexOf('=== PHASE 2:'));
    const result = await replay.accept(response(phase1));
    expect(result.screenCols).toBeUndefined();
    const lines = result.text.split('\n').filter(line => line.includes('record='));
    expect(lines).toHaveLength(25);
    for (let i = 0; i < lines.length; i++) {
      expect(lines[i]).toContain(`demo.Application : record=${i + 1} `);
      expect(lines[i]).toContain('message=ABCDEFGHIJKLMNOPQRSTUVWXYZ-0123456789-abcdefghijklmnopqrstuvwxyz');
    }
    expect(result.ansi).toContain('\x1b[0;38;5;2m');
    expect(result.ansi).not.toMatch(/\x1b\[[\d;]*H/);
    replay.dispose();
  });

  it('produces the same state for a snapshot and arbitrarily split control sequences', async () => {
    const full = new BackgroundTerminalReplay();
    const split = new BackgroundTerminalReplay();
    const expected = await full.accept(response(recording));
    let actual = await split.accept(response(''));
    let cursor = 2;
    for (let offset = 0; offset < recording.length;) {
      const length = (offset * 17 % 113) + 1;
      actual = await split.accept(response(recording.slice(offset, offset + length), cursor++, false));
      offset += length;
    }
    expect(actual).toEqual(expected);
    expect(actual.text).toContain('=== DONE: OUTPUT SHOULD END HERE ===');
    expect(actual.text).not.toContain('frame 20');
    full.dispose(); split.dispose();
  });

  it('reflows parsed log cells at different display widths without changing log content', async () => {
    const replay = new BackgroundTerminalReplay();
    const projection = await replay.accept(response(recording.slice(0, recording.indexOf('=== PHASE 2:'))));
    for (const cols of [60, 80, 120, 160]) {
      const display = new DisplayTerminal({ cols, rows: 35, scrollback: 5000, allowProposedApi: true });
      await new Promise<void>(resolve => display.write(projection.ansi, resolve));
      let text = '';
      const buffer = display.buffer.active;
      for (let i = 0; i < buffer.length; i++) {
        const line = buffer.getLine(i)!;
        if (i && !line.isWrapped) text += '\n';
        text += line.translateToString(!buffer.getLine(i + 1)?.isWrapped);
      }
      expect(text.trimEnd()).toBe(projection.text.trimEnd());
      display.dispose();
    }
    replay.dispose();
  });

  it('handles overwrite, colors, wide characters and alternate screen restoration', async () => {
    const replay = new BackgroundTerminalReplay();
    let result = await replay.accept(response('progress 0%\r\x1b[2Kprogress 100%\r\n\x1b[32m中文🙂\x1b[0m'));
    expect(result.text).toBe('progress 100%\n中文🙂');
    result = await replay.accept(response('\x1b[?1049h\x1b[2J\x1b[3;5HGRID', 2, false));
    expect(result.screenCols).toBe(80);
    expect(result.text).toBe('\n\n    GRID');
    result = await replay.accept(response('\x1b[?1049l', 3, false));
    expect(result.text).toBe('progress 100%\n中文🙂');
    expect(replay.rawOutput).toContain('\x1b[3;5H');
    replay.dispose();
  });

  it('ignores duplicate cursors and starts a fresh state on explicit reset', async () => {
    const replay = new BackgroundTerminalReplay();
    await replay.accept(response('one'));
    const first = await replay.accept(response('two', 2, false));
    expect(await replay.accept(response('two', 2, false))).toEqual(first);
    expect(await replay.accept({ ...response('replacement', 3), reset: true }))
      .toMatchObject({ text: 'replacement' });
    replay.dispose();
  });

  it('preserves painted blank cells instead of dropping their background', async () => {
    const replay = new BackgroundTerminalReplay();
    const result = await replay.accept(response('\x1b[41m   \x1b[0m'));
    expect(result.text).toBe('   ');
    expect(result.ansi).toContain('\x1b[0;48;5;1m   ');
    replay.dispose();
  });

  it('joins a wide-character wrap without inventing a padding space', async () => {
    const replay = new BackgroundTerminalReplay();
    const text = `${'x'.repeat(79)}中文🙂`;
    expect(await replay.accept(response(text))).toMatchObject({ text });
    replay.dispose();
  });

  it('marks truncated snapshots incomplete but keeps a continuously attached parser valid', async () => {
    const replay = new BackgroundTerminalReplay();
    const truncated = { ...metadata, truncatedFromStart: true };
    await replay.accept(response('first'));
    expect(await replay.accept({ ...response(' next', 2, false), metadata: truncated }))
      .toMatchObject({ text: 'first next', incomplete: false });
    expect(await replay.accept({ ...response('tail', 3), reset: true, metadata: truncated }))
      .toMatchObject({ text: 'tail', incomplete: true });
    replay.dispose();
  });

  it('supports legacy local/remote metadata and explicitly rejects unknown source geometry', async () => {
    for (const remote of [false, true]) {
      expect(sourceTerminalSize({ ...metadata, remote, terminalSize: undefined }))
        .toEqual({ cols: 80, rows: 24 });
    }
    for (const terminalSize of [null, { cols: 0, rows: 24 }, { cols: 80, rows: NaN }]) {
      const replay = new BackgroundTerminalReplay();
      expect(await replay.accept({ ...response('raw'), metadata: { ...metadata, terminalSize } }))
        .toMatchObject({ unknownGeometry: true, text: '' });
      expect(replay.rawOutput).toBe('raw');
      replay.dispose();
    }
  });
});
