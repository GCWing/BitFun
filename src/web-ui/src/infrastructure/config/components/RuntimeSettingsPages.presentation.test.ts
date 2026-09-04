import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  ).replace(/\r\n?/g, '\n');
}

describe('Runtime settings information architecture', () => {
  it('keeps execution and permissions unified and stacks browser and desktop control in one owner', () => {
    const source = readSource('./RuntimeSettingsPages.tsx');
    const appearance = readSource('./RuntimeSettingsPages.appearance.ts');
    const wrapper = readSource('../../../app/scenes/settings/pages/ExecutionSettingsPage.tsx');

    expect(wrapper).toContain('<RuntimeExecutionSettingsPage />');
    expect(wrapper).not.toContain('<SettingsViewPage');
    expect(wrapper).not.toContain("id: 'common' as const");
    expect(wrapper).not.toContain("id: 'advanced' as const");
    expect(source.match(/\{showsExecutionSettings \? \(/g)).toHaveLength(1);
    expect(source).toContain('export function ExecutionSettingsPage()');
    expect(source).not.toContain('ExecutionCommonSettingsPage');
    expect(source).not.toContain('ExecutionAdvancedSettingsPage');
    expect(source.indexOf("title={t('permissionPolicy.sectionTitle')}")).toBeLessThan(
      source.indexOf("title={t('toolExecution.sectionTitle')}"),
    );
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
    expect(source.match(/onClick=\{\(\) => void refreshComputerUseStatus\(\)\}/g)).toHaveLength(1);
    expect(source).toContain('extra={IS_TAURI_DESKTOP && !peerBrowserControlUnsupported ? (');
    expect(source).toContain('<ConfigRefreshButton');
    expect(source.indexOf("title={t('computerUse.sectionTitle')}")).toBeLessThan(
      source.indexOf("title={t('browserControl.sectionTitle')}"),
    );
    expect(appearance).toContain("'execution'");
    expect(appearance).not.toContain("'execution-common'");
    expect(appearance).not.toContain("'execution-advanced'");
    expect(appearance).toContain("'browser-desktop-control'");
    expect(appearance).not.toContain("'desktop-control'");
    expect(appearance).not.toContain("'browser-control'");
  });

  it('presents pet choices as an always-visible package-style card gallery', () => {
    const source = readSource('./RuntimeSettingsPages.tsx');
    const styles = readSource('./RuntimeSettingsPages.scss');

    expect(source).toContain('className="openbitfun-runtime-settings__pet-gallery"');
    expect(source).toContain('data-testid="companion-pet-card"');
    expect(source).toContain('openbitfun-runtime-settings__pet-selected-mark');
    expect(source).toContain('bodySurface={false}');
    expect(source).toContain('const hasLoadedPageDataRef = useRef(false);');
    expect(source).toContain('const reloadCompanionPets = useCallback(async () => {');
    expect(source).toContain("if (page === 'pet' && !isActive) return;");
    expect(source.match(/await reloadCompanionPets\(\);/g)).toHaveLength(2);
    expect(source).not.toContain('handleRefreshCompanionPets');
    expect(source).not.toContain('companionPetsLoading');
    expect(source).not.toContain('features.pet.refresh');
    expect(source).not.toContain('companionPetListExpanded');
    expect(source).not.toContain('openbitfun-runtime-settings__pet-expand-button');
    expect(source).not.toContain('openbitfun-runtime-settings__pet-preview-popover');
    expect(source).not.toContain('aria-expanded=');
    expect(styles).toContain('grid-template-columns: repeat(3, minmax(0, 1fr))');
    expect(styles).toContain('&__pet-card-preview');
    expect(styles).toContain('&__pet-selected-mark');
    expect(styles).toContain('border: 1px solid transparent;');
    expect(styles).toContain('&__pet-card:hover,\n  &__pet-card:focus-within {\n    border-color: var(--openbitfun-color-border-default);');
    expect(styles).not.toContain('&__pet-preview-popover');
    expect(styles).toContain(":root[data-openbitfun-appearance-mode='light'] &");
  });

  it('uses the app confirmation owner before deleting an imported pet', () => {
    const source = readSource('./RuntimeSettingsPages.tsx');

    expect(source).toContain("const confirmed = await confirmDanger(\n      t('features.pet.deleteConfirmTitle'),\n      t('features.pet.deleteConfirmBody'),");
    expect(source).not.toContain("import { ask, open } from '@tauri-apps/plugin-dialog'");
    expect(source).not.toContain("await ask(t('features.pet.deleteConfirmBody')");
  });

  it('keeps the desktop-control platform note aligned as a distinct card footer', () => {
    const source = readSource('./RuntimeSettingsPages.tsx');
    const styles = readSource('./RuntimeSettingsPages.scss');

    expect(source).toContain('className="openbitfun-runtime-settings__platform-note-icon"');
    expect(source).toContain('className="openbitfun-runtime-settings__platform-note-copy"');
    expect(source).not.toContain("padding: '8px 0 4px'");
    expect(styles).toContain('padding: var(--openbitfun-space-3) var(--openbitfun-space-5);');
    expect(styles).toContain('border-top: 1px solid var(--openbitfun-component-config-page-divider);');
    expect(styles).toContain('padding-inline: var(--openbitfun-space-4);');
  });
});
