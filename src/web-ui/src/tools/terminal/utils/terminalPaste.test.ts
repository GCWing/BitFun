import { describe, expect, it, vi } from 'vitest';

import {
  POWERSHELL_READLINE_PASTE_SEQUENCE,
  analyzeTerminalPaste,
  buildTerminalPastePreview,
  resolveTerminalPaste,
  shouldUsePowerShellReadlinePaste,
} from './terminalPaste';

describe('terminal paste policy', () => {
  it('allows single-line text without confirmation', () => {
    expect(analyzeTerminalPaste('echo hello')).toEqual({
      shouldConfirm: false,
      lineCount: 1,
      preview: 'echo hello',
      text: 'echo hello',
    });
  });

  it('strips a trailing blank line in auto mode to avoid immediate execution', () => {
    expect(analyzeTerminalPaste('echo hello\n')).toMatchObject({
      shouldConfirm: false,
      lineCount: 2,
      text: 'echo hello',
    });

    expect(analyzeTerminalPaste('echo hello\n\t')).toMatchObject({
      shouldConfirm: false,
      lineCount: 2,
      text: 'echo hello',
    });
  });

  it('allows multi-line text without confirmation when bracketed paste mode is active', () => {
    expect(analyzeTerminalPaste('echo one\necho two', { bracketedPasteMode: true })).toMatchObject({
      shouldConfirm: false,
      lineCount: 2,
      text: 'echo one\necho two',
    });
  });

  it('asks for confirmation for true multi-line text in auto mode', () => {
    expect(analyzeTerminalPaste('echo one\necho two')).toMatchObject({
      shouldConfirm: true,
      lineCount: 2,
      text: 'echo one\necho two',
    });
  });

  it('honors always and never warning modes', () => {
    expect(analyzeTerminalPaste('echo hello\n', { warningMode: 'always' })).toMatchObject({
      shouldConfirm: true,
      text: 'echo hello\n',
    });

    expect(analyzeTerminalPaste('echo one\necho two', { warningMode: 'never' })).toMatchObject({
      shouldConfirm: false,
      text: 'echo one\necho two',
    });
  });

  it('builds a compact VS Code-style preview', () => {
    expect(buildTerminalPastePreview([
      '012345678901234567890123456789extra',
      'second',
      'third',
      'fourth',
    ])).toBe('012345678901234567890123456789...\nsecond\nthird\n...');
  });

  it('can convert confirmed multi-line paste to one line', async () => {
    const confirmMultiLinePaste = vi.fn().mockResolvedValue('pasteAsSingleLine');

    await expect(resolveTerminalPaste('echo one\necho two', {
      confirmMultiLinePaste,
    })).resolves.toEqual({
      allow: true,
      text: 'echo oneecho two',
    });

    expect(confirmMultiLinePaste).toHaveBeenCalledWith({
      lineCount: 2,
      preview: 'echo one\necho two',
    });
  });

  it('detects Windows PowerShell sessions that should delegate Ctrl+V to PSReadLine', () => {
    expect(POWERSHELL_READLINE_PASTE_SEQUENCE).toBe('\x16');
    expect(shouldUsePowerShellReadlinePaste('PowerShell', { isWindowsClient: true })).toBe(true);
    expect(shouldUsePowerShellReadlinePaste('pwsh', { isWindowsClient: true })).toBe(true);
    expect(shouldUsePowerShellReadlinePaste('PowerShellCore', { isWindowsClient: true })).toBe(true);
    expect(shouldUsePowerShellReadlinePaste('Bash', { isWindowsClient: true })).toBe(false);
    expect(shouldUsePowerShellReadlinePaste('PowerShell', { isWindowsClient: false })).toBe(false);
  });
});
