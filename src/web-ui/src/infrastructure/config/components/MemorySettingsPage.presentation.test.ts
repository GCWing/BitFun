import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(): string {
  return readFileSync(
    fileURLToPath(new URL('./MemorySettingsPage.tsx', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('Memory settings presentation', () => {
  it('uses the shared settings section for advanced controls', () => {
    const source = readSource();
    const advancedStart = source.indexOf("title={t('sections.advanced.title')}");
    const advancedEnd = source.indexOf('</ConfigPageSection>', advancedStart);
    const advancedSection = source.slice(advancedStart, advancedEnd);

    expect(advancedStart).toBeGreaterThan(-1);
    expect(advancedEnd).toBeGreaterThan(advancedStart);
    expect(source).not.toContain("import './MemoriesConfig.scss';");
    expect(source).not.toContain('<details');
    expect(advancedSection).toContain("description={t('sections.advanced.description')}");
    expect(advancedSection).toContain('aria-expanded={advancedOpen}');
    expect(advancedSection).toContain('{advancedOpen && (');
  });

  it('groups header actions in a labelled menu and keeps clearing memory destructive', () => {
    const source = readSource();

    expect(source).toContain('<MenuPopover');
    expect(source).toContain("{t('actions.menu')}");
    expect(source).toContain("id: 'open-directory'");
    expect(source).toContain("id: 'reset-settings'");
    expect(source).toContain("id: 'clear-memory'");
    expect(source).toContain("tone: 'danger'");
  });
});
