/**
 * @vitest-environment jsdom
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shortcutManager } from '@/infrastructure/services/ShortcutManager';

describe('grid9 chat-scope reachability', () => {
  beforeEach(() => {
    shortcutManager.clear();
  });

  it('fires canvas.splitGrid9.chat when focus is in chat scope', () => {
    const cb = vi.fn();
    shortcutManager.register('canvas.splitGrid9.chat', { key: '9', ctrl: true, shift: true, scope: 'chat' }, cb);
    const target = document.createElement('div');
    target.setAttribute('data-shortcut-scope', 'chat');
    document.body.appendChild(target);
    target.dispatchEvent(new KeyboardEvent('keydown', {
      key: '9', code: 'Digit9', ctrlKey: true, shiftKey: true, bubbles: true, cancelable: true,
    }));
    expect(cb).toHaveBeenCalled();
    document.body.removeChild(target);
  });

  it('does NOT fire when focus is in canvas scope and only chat registered', () => {
    const cb = vi.fn();
    shortcutManager.register('canvas.splitGrid9.chat', { key: '9', ctrl: true, shift: true, scope: 'chat' }, cb);
    const target = document.createElement('div');
    target.setAttribute('data-shortcut-scope', 'canvas');
    document.body.appendChild(target);
    target.dispatchEvent(new KeyboardEvent('keydown', {
      key: '9', code: 'Digit9', ctrlKey: true, shiftKey: true, bubbles: true, cancelable: true,
    }));
    expect(cb).not.toHaveBeenCalled();
    document.body.removeChild(target);
  });

  it('checkConflicts reports zero conflicts for Ctrl+Shift+9 in canvas scope', () => {
    const conflicts = shortcutManager.checkConflicts({ key: '9', ctrl: true, shift: true, scope: 'canvas' });
    expect(conflicts).toHaveLength(0);
  });

  it('checkConflicts reports zero conflicts for Ctrl+Shift+9 in chat scope', () => {
    const conflicts = shortcutManager.checkConflicts({ key: '9', ctrl: true, shift: true, scope: 'chat' });
    expect(conflicts).toHaveLength(0);
  });
});
