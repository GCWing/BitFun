import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSettingsNavStylesheet(): string {
  return readFileSync(
    fileURLToPath(new URL('./SettingsNav.scss', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('SettingsNav typography', () => {
  it('uses the main navigation font scope and semantic text roles', () => {
    const stylesheet = readSettingsNavStylesheet();

    expect(stylesheet).toContain("@use '../../styles/nav-panel-font-scope.scss' as nav-font;");
    expect(stylesheet).toContain('@include nav-font.nav-panel-font-token-scope;');
    expect(stylesheet).toContain('@include nav-font.nav-panel-text-body;');
    expect(stylesheet).toContain('@include nav-font.nav-panel-text-heading;');
    expect(stylesheet).toContain('@include nav-font.nav-panel-text-meta;');
    expect(stylesheet).toContain('font-family: inherit;');
  });

  it('matches the main navigation category and item reading rhythm', () => {
    const stylesheet = readSettingsNavStylesheet();

    expect(stylesheet).toContain('font-size: var(--bf-appearance-token-font-size-xs);\n    font-weight: 500;\n    letter-spacing: 0.015em;\n    line-height: 1.25;');
    expect(stylesheet).toContain('font-size: var(--bf-appearance-token-font-size-sm);\n    font-weight: 400;\n    line-height: 1.25;');
    expect(stylesheet).toContain('&.is-active {\n      @include nav-font.nav-panel-text-heading;\n\n      background: var(--bf-appearance-token-element-bg-soft);\n      font-weight: 600;');
  });
});
