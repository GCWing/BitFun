import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  );
}

describe('DevelopmentSettingsPage structure', () => {
  it('keeps terminal and editor in one continuous page without tabs', () => {
    const source = readSource('./DevelopmentSettingsPage.tsx');
    const terminalIndex = source.indexOf('<TerminalSettingsPage />');
    const editorIndex = source.indexOf('<EditorConfig />');

    expect(terminalIndex).toBeGreaterThan(-1);
    expect(editorIndex).toBeGreaterThan(terminalIndex);
    expect(source).not.toContain('SettingsViewPage');
    expect(source).not.toContain('<Tabs');
    expect(source).not.toContain('<TabPane');
    expect(source).toContain("lazy(() => import('@/infrastructure/config/components/EditorConfig'))");
    expect(source).toContain("lazy(() => import('@/infrastructure/config/components/ApplicationSettingsPages')");
  });

  it('separates terminal and editor with whitespace instead of a divider', () => {
    const styles = readSource('./DevelopmentSettingsPage.scss');
    const editorRule = styles.match(/&--editor\s*{([^}]*)}/)?.[1] ?? '';

    expect(editorRule).toContain('margin-top');
    expect(editorRule).not.toContain('border');
  });
});
