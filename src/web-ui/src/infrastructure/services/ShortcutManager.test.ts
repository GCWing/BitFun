/**
 * @vitest-environment jsdom
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { EDITOR_SHORTCUTS } from '@/shared/constants/shortcuts';
import { shortcutManager } from './ShortcutManager';

function setPlatform(platform: string): void {
  Object.defineProperty(window.navigator, 'platform', {
    value: platform,
    configurable: true,
  });
}

function dispatchScopedKey(scope: string, init: KeyboardEventInit): void {
  const target = document.createElement('div');
  target.setAttribute('data-shortcut-scope', scope);
  document.body.appendChild(target);
  target.dispatchEvent(new KeyboardEvent('keydown', {
    key: init.key,
    code: init.code,
    ctrlKey: init.ctrlKey,
    metaKey: init.metaKey,
    shiftKey: init.shiftKey,
    altKey: init.altKey,
    keyCode: init.keyCode,
    bubbles: true,
    cancelable: true,
  }));
  target.remove();
}

describe('ShortcutManager platform primary modifier', () => {
  beforeEach(() => {
    shortcutManager.clear();
    shortcutManager.setEnabled(true);
    shortcutManager.loadUserOverrides({});
    document.body.innerHTML = '';
  });

  afterEach(() => {
    shortcutManager.clear();
    vi.restoreAllMocks();
  });

  it('maps logical Ctrl shortcuts to Command on macOS', () => {
    setPlatform('MacIntel');
    const callback = vi.fn();
    shortcutManager.register(
      'editor.findInFile',
      { key: 'f', ctrl: true, scope: 'editor', allowInInput: true },
      callback
    );

    dispatchScopedKey('editor', { key: 'f', metaKey: true });

    expect(callback).toHaveBeenCalledTimes(1);
  });

  it('does not treat physical Control as the macOS primary modifier', () => {
    setPlatform('MacIntel');
    const callback = vi.fn();
    shortcutManager.register(
      'editor.findInFile',
      { key: 'f', ctrl: true, scope: 'editor', allowInInput: true },
      callback
    );

    dispatchScopedKey('editor', { key: 'f', ctrlKey: true });

    expect(callback).not.toHaveBeenCalled();
  });

  it('keeps shortcut catalog defaults platform-neutral', () => {
    const findInFile = EDITOR_SHORTCUTS.find((shortcut) => shortcut.id === 'editor.findInFile');

    expect(findInFile?.config).toMatchObject({ key: 'f', ctrl: true });
    expect(findInFile?.config.meta).toBeUndefined();
  });

  it('detects app-scope conflicts against scoped shortcuts', () => {
    setPlatform('Win32');
    shortcutManager.register('app.search', { key: 'k', ctrl: true, scope: 'app' }, vi.fn());
    shortcutManager.register('chat.search', { key: 'k', ctrl: true, scope: 'chat' }, vi.fn());

    expect(shortcutManager.checkConflicts({ key: 'k', ctrl: true, scope: 'chat' }, 'chat.search'))
      .toEqual([expect.objectContaining({ id: 'app.search' })]);
    expect(shortcutManager.checkConflicts({ key: 'k', ctrl: true, scope: 'app' }, 'app.search'))
      .toEqual([expect.objectContaining({ id: 'chat.search' })]);
  });

  it('detects Ctrl and Meta as the same primary modifier on macOS conflicts', () => {
    setPlatform('MacIntel');
    shortcutManager.register('app.find', { key: 'f', meta: true, scope: 'app' }, vi.fn());

    expect(shortcutManager.checkConflicts({ key: 'f', ctrl: true, scope: 'editor' }))
      .toEqual([expect.objectContaining({ id: 'app.find' })]);
  });

  it('does not run Escape shortcuts while IME owns the key', () => {
    const callback = vi.fn();
    shortcutManager.register(
      'chat.stopGeneration',
      { key: 'Escape', scope: 'chat', allowInInput: true },
      callback
    );

    dispatchScopedKey('chat', { key: 'Escape', keyCode: 229 } as KeyboardEventInit);

    expect(callback).not.toHaveBeenCalled();
  });

  it('does not inherit canvas shortcuts when focus is inside terminal scope', () => {
    const canvasCallback = vi.fn();
    const terminalCallback = vi.fn();
    shortcutManager.register(
      'canvas.closePreview',
      { key: 'Escape', scope: 'canvas', allowInInput: true },
      canvasCallback
    );
    shortcutManager.register(
      'terminal.escape',
      { key: 'Escape', scope: 'terminal', allowInInput: true },
      terminalCallback
    );

    dispatchScopedKey('terminal', { key: 'Escape' });

    expect(canvasCallback).not.toHaveBeenCalled();
    expect(terminalCallback).toHaveBeenCalledTimes(1);
  });
});
