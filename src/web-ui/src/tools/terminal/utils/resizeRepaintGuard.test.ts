import { describe, expect, it } from 'vitest';

import {
  ResizeRepaintGuard,
  type ResizeRepaintScreenSnapshot,
} from './resizeRepaintGuard';

function createScreenSnapshot(overrides: Partial<ResizeRepaintScreenSnapshot> = {}): ResizeRepaintScreenSnapshot {
  return {
    bufferType: 'normal',
    cols: 170,
    rows: 22,
    viewportY: 0,
    baseY: 0,
    visibleNonEmptyLines: [
      'PS C:\\Workspace\\Project> ls',
      '    Directory: C:\\Workspace\\Project',
      'Mode                 LastWriteTime         Length Name',
      '----                 -------------         ------ ----',
      'd----           2026/6/14    14:50                assets',
      'PS C:\\Workspace\\Project>',
    ],
    ...overrides,
  };
}

function createCmdScreenSnapshot(overrides: Partial<ResizeRepaintScreenSnapshot> = {}): ResizeRepaintScreenSnapshot {
  return {
    bufferType: 'normal',
    cols: 170,
    rows: 26,
    viewportY: 0,
    baseY: 0,
    visibleNonEmptyLines: [
      'Microsoft Windows [Version 10.0.26200.0000]',
      '(c) Microsoft Corporation. All rights reserved.',
      'C:\\Workspace\\Project>dir',
      ' Directory of C:\\Workspace\\Project',
      '2026/06/14  14:50    <DIR>          assets',
      '2026/06/14  14:51    <DIR>          css',
      '2026/06/14  14:51             1,164 index.html',
      '2026/06/14  14:51    <DIR>          js',
      '               1 File(s)          1,164 bytes',
      'C:\\Workspace\\Project>',
    ],
    ...overrides,
  };
}

function createGitBashScreenSnapshot(overrides: Partial<ResizeRepaintScreenSnapshot> = {}): ResizeRepaintScreenSnapshot {
  return {
    bufferType: 'normal',
    cols: 170,
    rows: 28,
    viewportY: 0,
    baseY: 0,
    visibleNonEmptyLines: [
      'dev@WORKSTATION MINGW64 /c/Workspace/Project',
      '$ ll',
      'total 4',
      'drwxr-xr-x 1 dev 197609    0  Jun 14 14:50 assets/',
      'drwxr-xr-x 1 dev 197609    0  Jun 14 14:51 css/',
      '-rw-r--r-- 1 dev 197609 1164  Jun 14 14:51 index.html',
      'drwxr-xr-x 1 dev 197609    0  Jun 14 14:51 js/',
      'dev@WORKSTATION MINGW64 /c/Workspace/Project',
      '$',
    ],
    ...overrides,
  };
}

describe('ResizeRepaintGuard', () => {
  it('suppresses an incomplete Windows resize repaint that would overwrite the top line', () => {
    const guard = new ResizeRepaintGuard();
    guard.markResize({
      cols: 170,
      rows: 22,
      previousCols: 170,
      previousRows: 11,
      shellType: 'PowerShell',
      screen: createScreenSnapshot(),
      nowMs: 1000,
    });

    const repaint = [
      '\x1b[?25l\x1b[H\x1b[K\r\n',
      '    Directory: C:\\Workspace\\Project\x1b[K\r\n',
      '\x1b[K\x1b[32m\x1b[1m\r\n',
      'Mode                 LastWriteTime\x1b[m \x1b[32m\x1b[1m\x1b[3m        Length\x1b[23m Name\x1b[m\x1b[K\r\n',
      '----                 -------------         ------ ----\x1b[K\r\n',
      'd----           2026/6/14    14:50                assets\x1b[K',
    ].join('');

    const decision = guard.inspect(repaint, 1090);

    expect(decision.suppress).toBe(true);
    if (decision.suppress) {
      expect(decision.details.reason).toBe('incomplete-resize-repaint');
      expect(decision.details.topLine).toBe('ps c:\\workspace\\project> ls');
      expect(decision.details.matchingLines).toContain('directory: c:\\workspace\\project');
    }
  });

  it('allows a repaint that includes the current top line', () => {
    const guard = new ResizeRepaintGuard();
    guard.markResize({
      cols: 170,
      rows: 22,
      screen: createScreenSnapshot(),
      nowMs: 1000,
    });

    const decision = guard.inspect(
      '\x1b[H\x1b[KPS C:\\Workspace\\Project> ls\r\n    Directory: C:\\Workspace\\Project',
      1050,
    );

    expect(decision).toEqual({ suppress: false, reason: 'repaint-starts-with-top-line' });
  });

  it('suppresses a Cmd resize repaint that starts with the old small viewport', () => {
    const guard = new ResizeRepaintGuard();
    guard.markResize({
      cols: 170,
      rows: 26,
      previousCols: 170,
      previousRows: 6,
      shellType: 'Cmd',
      screen: createCmdScreenSnapshot(),
      nowMs: 1000,
    });

    const repaint = [
      '\x1b[?25l\x1b[H',
      '2026/06/14  14:51             1,164 index.html\x1b[K\r\n',
      '2026/06/14  14:51    <DIR>          js\x1b[K\r\n',
      '               1 File(s)          1,164 bytes\x1b[K\r\n',
      '\x1b[K\r\n',
      'C:\\Workspace\\Project>\x1b[K\r\n',
      '\x1b[K\r\n\x1b[K\x1b[6;22H\x1b[?25h',
    ].join('');

    const decision = guard.inspect(repaint, 1075);

    expect(decision.suppress).toBe(true);
    if (decision.suppress) {
      expect(decision.details.reason).toBe('incomplete-resize-repaint');
      expect(decision.details.topLine).toBe('microsoft windows [version 10.0.26200.0000]');
      expect(decision.details.matchingLines).toContain('2026/06/14 14:51 1,164 index.html');
    }
  });

  it('allows a Cmd resize repaint that matches the current small viewport top line', () => {
    const guard = new ResizeRepaintGuard();
    guard.markResize({
      cols: 170,
      rows: 6,
      previousCols: 170,
      previousRows: 26,
      shellType: 'Cmd',
      screen: createCmdScreenSnapshot({
        rows: 6,
        viewportY: 13,
        baseY: 13,
        visibleNonEmptyLines: [
          '2026/06/14  14:51             1,164 index.html',
          '2026/06/14  14:51    <DIR>          js',
          '               1 File(s)          1,164 bytes',
          'C:\\Workspace\\Project>',
        ],
      }),
      nowMs: 1000,
    });

    const decision = guard.inspect(
      '\x1b[?25l\x1b[H2026/06/14  14:51             1,164 index.html\x1b[K\r\n2026/06/14  14:51    <DIR>          js\x1b[K',
      1075,
    );

    expect(decision).toEqual({ suppress: false, reason: 'repaint-starts-with-top-line' });
  });

  it('suppresses a Git Bash resize repaint even when the top prompt appears later', () => {
    const guard = new ResizeRepaintGuard();
    guard.markResize({
      cols: 170,
      rows: 28,
      previousCols: 170,
      previousRows: 6,
      shellType: 'Bash',
      screen: createGitBashScreenSnapshot(),
      nowMs: 1000,
    });

    const repaint = [
      '\x1b[?25l\x1b[H',
      'drwxr-xr-x 1 dev 197609    0  Jun 14 14:51 \x1b[34m\x1b[1mcss\x1b[m/\x1b[K\r\n',
      '-rw-r--r-- 1 dev 197609 1164  Jun 14 14:51 index.html\x1b[K\r\n',
      'drwxr-xr-x 1 dev 197609    0  Jun 14 14:51 \x1b[34m\x1b[1mjs\x1b[m/\x1b[K\r\n',
      '\x1b[K\x1b[32m\r\n',
      'dev@WORKSTATION \x1b[35mMINGW64 \x1b[33m/c/Workspace/Project\x1b[K\x1b[m\r\n',
      '$\x1b[K\r\n',
      '\x1b[K\r\n\x1b[K\x1b[?25h',
    ].join('');

    const decision = guard.inspect(repaint, 1120);

    expect(decision.suppress).toBe(true);
    if (decision.suppress) {
      expect(decision.details.reason).toBe('incomplete-resize-repaint');
      expect(decision.details.topLine).toBe('dev@workstation mingw64 /c/workspace/project');
      expect(decision.details.matchingLines).toContain('drwxr-xr-x 1 dev 197609 0 jun 14 14:51 css/');
    }
  });

  it('allows a Git Bash repaint that starts with the current prompt', () => {
    const guard = new ResizeRepaintGuard();
    guard.markResize({
      cols: 170,
      rows: 28,
      previousCols: 170,
      previousRows: 6,
      shellType: 'Bash',
      screen: createGitBashScreenSnapshot(),
      nowMs: 1000,
    });

    const decision = guard.inspect(
      '\x1b[?25l\x1b[Hdev@WORKSTATION \x1b[35mMINGW64 \x1b[33m/c/Workspace/Project\x1b[K\x1b[m\r\n$ ll\x1b[K\r\ntotal 4\x1b[K',
      1120,
    );

    expect(decision).toEqual({ suppress: false, reason: 'repaint-starts-with-top-line' });
  });

  it('does not suppress alternate-buffer repaint output', () => {
    const guard = new ResizeRepaintGuard();
    guard.markResize({
      cols: 170,
      rows: 22,
      screen: createScreenSnapshot({ bufferType: 'alternate' }),
      nowMs: 1000,
    });

    const decision = guard.inspect(
      '\x1b[H\x1b[K    Directory: C:\\Workspace\\Project\r\nMode                 LastWriteTime         Length Name',
      1050,
    );

    expect(decision).toEqual({ suppress: false, reason: 'alternate-buffer' });
  });

  it('expires the repaint guard window', () => {
    const guard = new ResizeRepaintGuard();
    guard.markResize({
      cols: 170,
      rows: 22,
      screen: createScreenSnapshot(),
      nowMs: 1000,
    });

    const decision = guard.inspect(
      '\x1b[H\x1b[K    Directory: C:\\Workspace\\Project\r\nMode                 LastWriteTime         Length Name',
      2000,
    );

    expect(decision).toEqual({ suppress: false, reason: 'expired' });
  });

  it('allows ordinary output inside the resize window', () => {
    const guard = new ResizeRepaintGuard();
    guard.markResize({
      cols: 170,
      rows: 22,
      screen: createScreenSnapshot(),
      nowMs: 1000,
    });

    const decision = guard.inspect('hello from the terminal\r\n', 1050);

    expect(decision).toEqual({ suppress: false, reason: 'no-home-repaint-prefix' });
  });
});
