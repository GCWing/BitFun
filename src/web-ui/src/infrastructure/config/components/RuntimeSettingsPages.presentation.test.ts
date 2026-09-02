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
  it('keeps execution separate and stacks browser and desktop control in one owner', () => {
    const source = readSource('./RuntimeSettingsPages.tsx');
    const appearance = readSource('./RuntimeSettingsPages.appearance.ts');
    const wrapper = readSource('../../../app/scenes/settings/pages/ExecutionSettingsPage.tsx');

    expect(wrapper).toContain('<SettingsViewPage');
    expect(wrapper).toContain("id: 'common' as const");
    expect(wrapper).toContain("id: 'advanced' as const");
    expect(source.match(/\{page === 'browser-desktop-control' \? \(/g)).toHaveLength(1);
    expect(source).toContain('export function BrowserDesktopControlSettingsPage()');
    expect(source).not.toContain('export function DesktopControlSettingsPage()');
    expect(source).not.toContain('export function BrowserControlSettingsPage()');
    expect(source).not.toContain("page === 'execution-control'");
    expect(source).not.toContain("page === 'desktop-control'");
    expect(source).not.toContain("page === 'browser-control'");
    expect(source).not.toContain('refreshDesktopStatus');
    expect(source).toContain("if (page === 'browser-desktop-control') {");
    expect(source).toContain('void refreshComputerUseStatus();');
    expect(source).toContain('void refreshBrowserControlStatus();');
    expect(source.indexOf("title={t('computerUse.sectionTitle')}")).toBeLessThan(
      source.indexOf("title={t('browserControl.sectionTitle')}"),
    );
    expect(appearance).toContain("'execution-common'");
    expect(appearance).toContain("'execution-advanced'");
    expect(appearance).toContain("'browser-desktop-control'");
    expect(appearance).not.toContain("'desktop-control'");
    expect(appearance).not.toContain("'browser-control'");
  });

  it('presents pet choices as an always-visible package-style card gallery', () => {
    const source = readSource('./RuntimeSettingsPages.tsx');
    const styles = readSource('./RuntimeSettingsPages.scss');

    expect(source).toContain('className="bitfun-runtime-settings__pet-gallery"');
    expect(source).toContain('data-testid="companion-pet-card"');
    expect(source).toContain('bitfun-runtime-settings__pet-selected-mark');
    expect(source).toContain('bodySurface={false}');
    expect(source).toContain('const hasLoadedPageDataRef = useRef(false);');
    expect(source).toContain('const reloadCompanionPets = useCallback(async () => {');
    expect(source).toContain("if (page === 'pet' && !isActive) return;");
    expect(source.match(/await reloadCompanionPets\(\);/g)).toHaveLength(2);
    expect(source).not.toContain('handleRefreshCompanionPets');
    expect(source).not.toContain('companionPetsLoading');
    expect(source).not.toContain('features.pet.refresh');
    expect(source).not.toContain('companionPetListExpanded');
    expect(source).not.toContain('bitfun-runtime-settings__pet-expand-button');
    expect(source).not.toContain('aria-expanded=');
    expect(styles).toContain('grid-template-columns: repeat(3, minmax(0, 1fr))');
    expect(styles).toContain('&__pet-card-preview');
    expect(styles).toContain('&__pet-selected-mark');
    expect(styles).toContain(":root[data-bf-appearance-mode='light'] &");
  });

  it('keeps the desktop-control platform note aligned as a distinct card footer', () => {
    const source = readSource('./RuntimeSettingsPages.tsx');
    const styles = readSource('./RuntimeSettingsPages.scss');

    expect(source).toContain('className="bitfun-runtime-settings__platform-note-icon"');
    expect(source).toContain('className="bitfun-runtime-settings__platform-note-copy"');
    expect(source).not.toContain("padding: '8px 0 4px'");
    expect(styles).toContain('padding: var(--bf-space-3) var(--bf-space-5);');
    expect(styles).toContain('border-top: 1px solid var(--bf-component-config-page-divider);');
    expect(styles).toContain('padding-inline: var(--bf-space-4);');
  });
});
