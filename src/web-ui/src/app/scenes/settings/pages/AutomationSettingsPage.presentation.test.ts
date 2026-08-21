import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  );
}

describe('AutomationSettingsPage structure', () => {
  it('keeps quick actions and Hooks in one continuous page without tabs', () => {
    const source = readSource('./AutomationSettingsPage.tsx');
    const quickActionsIndex = source.indexOf('<QuickActionsConfig />');
    const hooksIndex = source.indexOf('<HooksConfig />');

    expect(quickActionsIndex).toBeGreaterThan(-1);
    expect(hooksIndex).toBeGreaterThan(quickActionsIndex);
    expect(source).not.toContain('SettingsViewPage');
    expect(source).not.toContain('<Tabs');
    expect(source).not.toContain('<TabPane');
  });

  it('separates quick actions and Hooks with whitespace instead of a divider', () => {
    const styles = readSource('./AutomationSettingsPage.scss');
    const hooksRule = styles.match(/&--hooks\s*{([^}]*)}/)?.[1] ?? '';

    expect(hooksRule).toContain('margin-top');
    expect(hooksRule).not.toContain('border');
  });
});
