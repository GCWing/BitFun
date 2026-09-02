import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  );
}

describe('Runtime settings information architecture', () => {
  it('keeps execution views and device controls in separate owners', () => {
    const source = readSource('./RuntimeSettingsPages.tsx');
    const appearance = readSource('./RuntimeSettingsPages.appearance.ts');
    const wrapper = readSource('../../../app/scenes/settings/pages/ExecutionSettingsPage.tsx');

    expect(wrapper).toContain('<SettingsViewPage');
    expect(wrapper).toContain("id: 'common' as const");
    expect(wrapper).toContain("id: 'advanced' as const");
    expect(source).toContain("{page === 'desktop-control' ? (");
    expect(source).toContain("{page === 'browser-control' ? (");
    expect(source).toContain('export function DesktopControlSettingsPage()');
    expect(source).toContain('export function BrowserControlSettingsPage()');
    expect(source).not.toContain("page === 'execution-control'");
    expect(source).not.toContain("page === 'device-control'");
    expect(source).not.toContain('refreshDesktopStatus');
    expect(source).toContain("if (page === 'desktop-control') {");
    expect(source).toContain("} else if (page === 'browser-control') {");
    expect(appearance).toContain("'execution-common'");
    expect(appearance).toContain("'execution-advanced'");
    expect(appearance).toContain("'desktop-control'");
    expect(appearance).toContain("'browser-control'");
  });

  it('keeps pet picker cards compact without description rows', () => {
    const source = readSource('./RuntimeSettingsPages.tsx');
    const styles = readSource('./RuntimeSettingsPages.scss');

    expect(source).toContain('bitfun-runtime-settings__pet-select-label');
    expect(source).not.toContain('bitfun-runtime-settings__pet-select-description');
    expect(styles).not.toContain('&__pet-select-description');
  });
});
