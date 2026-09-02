import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSibling(filename: string): string {
  return readFileSync(
    fileURLToPath(new URL(filename, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('Keyboard shortcuts design-system composition', () => {
  it('uses shared controls for shortcut badges and revert actions', () => {
    const source = readSibling('./KeyboardShortcutsTab.tsx');
    const stylesheet = readSibling('./KeyboardShortcutsTab.scss');
    const appearance = readSibling('./KeyboardShortcutsTab.appearance.ts');

    expect(source).toContain('IconButton');
    expect(source).toContain("variant={isRecording ? 'primary' : 'outline'}");
    expect(source).not.toContain('NON_USER_CUSTOMIZABLE_SHORTCUT_IDS');
    expect(source).not.toContain('KeyHint');
    expect(source).not.toMatch(/<(?:button|kbd)\b/);
    expect(stylesheet).not.toContain('kb-shortcuts__keybadge');
    expect(stylesheet).not.toContain('kb-shortcuts__revert-btn');
    expect(appearance).not.toMatch(/\{ id: '(?:keyBadge|revert)' \}/);
    expect(appearance).not.toContain("{ id: 'readonly'");
  });
});
