import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const source = (path: string) => readFileSync(resolve(process.cwd(), path), 'utf8');

describe('global search ownership', () => {
  it('mounts one shell-owned root and routes the nav trigger into its store', () => {
    expect(source('src/app/App.tsx')).toContain('<LazyGlobalSearchRoot />');
    expect(source('src/app/components/NavPanel/MainNav.tsx')).toContain('openGlobalSearch()');
    expect(source('src/app/components/NavPanel/MainNav.tsx')).not.toContain('NavSearchDialog');
  });

  it('reuses the shared search content in the session right-panel empty state', () => {
    const globalSearch = source('src/app/global-search/GlobalSearchRoot.tsx');
    const auxPane = source('src/app/scenes/session/AuxPane.tsx');
    const contentCanvas = source('src/app/components/panels/content-canvas/ContentCanvas.tsx');

    expect(globalSearch).toContain('export const GlobalSearchContent');
    expect(globalSearch).toContain('variant="modal"');
    expect(auxPane).toContain('emptyState={<GlobalSearchContent active={isSceneActive} variant="embedded" />}');
    expect(contentCanvas).toContain('<EmptyState onClose={disablePopOut ? undefined : collapsePanel}>');
  });

  it('keeps reference-specific command icon colors local to the modal presentation', () => {
    const globalSearch = source('src/app/global-search/GlobalSearchRoot.tsx');
    const globalSearchStyles = source('src/app/global-search/GlobalSearchRoot.scss');
    const actionCatalog = source('src/app/global-search/productActionCatalog.ts');

    expect(globalSearch).toContain('const MODAL_ACTION_ICON_TONES: Partial<Record<ProductActionId');
    expect(globalSearch).toContain("'session.new': 'red'");
    expect(globalSearch).toContain("'surface.browser.open': 'orange'");
    expect(globalSearch).toContain("'surface.terminal.open': 'green'");
    expect(globalSearch).toContain("'project.open': 'cyan'");
    expect(globalSearch).toContain("'project.new': 'blue'");
    expect(globalSearch).toContain("'surface.files.open': 'purple'");
    expect(globalSearch).toContain('variant === \'modal\' && itemVariant === \'action\'');
    expect(globalSearch).toContain('className="global-search__action-icon"');
    expect(globalSearch).toContain('data-icon-tone={modalActionIconTone}');
    expect(actionCatalog).toMatch(/id: 'session\.new',[\s\S]*?icon: 'message-circle'/);
    expect(actionCatalog).toMatch(/id: 'project\.open',[\s\S]*?icon: 'folder'/);
    expect(actionCatalog).toMatch(/id: 'project\.new',[\s\S]*?icon: 'plus'/);
    expect(actionCatalog).not.toMatch(/'folder-open'|'folder-plus'/);

    expect(globalSearchStyles).toMatch(
      /\.global-search--modal\s*\{[\s\S]*\.global-search__action-icon\s*\{/,
    );
    expect(globalSearchStyles).toContain('--_global-search-command-red: var(--bf-color-status-danger-content)');
    expect(globalSearchStyles).toContain('--_global-search-command-orange: var(--bf-color-status-warning-content)');
    expect(globalSearchStyles).toContain('--_global-search-command-green: var(--bf-color-status-success-content)');
    expect(globalSearchStyles).toContain('--_global-search-command-cyan: var(--bf-color-status-info-content)');
    expect(globalSearchStyles).toContain('--_global-search-command-blue: var(--bf-color-accent-default)');
    expect(globalSearchStyles).toContain('--_global-search-command-purple: var(--bf-color-accent-secondary)');
    expect(globalSearchStyles).toContain("&[data-icon-tone='red']");
    expect(globalSearchStyles).toContain("&[data-icon-tone='orange']");
    expect(globalSearchStyles).toContain("&[data-icon-tone='green']");
    expect(globalSearchStyles).toContain("&[data-icon-tone='cyan']");
    expect(globalSearchStyles).toContain("&[data-icon-tone='blue']");
    expect(globalSearchStyles).toContain("&[data-icon-tone='purple']");
    expect(actionCatalog).not.toMatch(/iconTone|iconColor|leadingTone/);
  });

  it('does not persist pointer hover as the keyboard-active result', () => {
    const globalSearch = source('src/app/global-search/GlobalSearchRoot.tsx');
    const globalSearchStyles = source('src/app/global-search/GlobalSearchRoot.scss');

    expect(globalSearch).not.toContain('onMouseEnter={() => setActiveId(item.id)}');
    expect(globalSearch).not.toContain('setActiveId(navigableItems[0].id)');
    expect(globalSearch).toContain("if (event.key === 'ArrowDown')");
    expect(globalSearch).toContain('setActiveId(navigableItems[nextIndex]?.id ?? null)');
    expect(globalSearchStyles).toMatch(
      /&:hover\s*\{\s*border-color: var\(--bf-color-border-default\)/,
    );
  });

  it('keeps browser and terminal capabilities on the shared product activator without footer shortcuts', () => {
    const footer = source('src/app/components/NavPanel/components/PersistentFooterActions.tsx');
    const activator = source('src/app/global-search/productActionActivator.ts');

    expect(footer).not.toContain('data-testid="browser-panel-entry"');
    expect(footer).not.toContain('data-testid="shell-panel-entry"');
    expect(activator).toContain("case 'surface.browser.open':");
    expect(activator).toContain("case 'surface.terminal.open':");
    expect(activator).toContain("new CustomEvent('terminal-create-requested')");
    expect(activator).not.toContain("openNavScene('shell')");
  });

  it('does not register a Shell navigation surface in the left panel', () => {
    const navRegistry = source('src/app/scenes/nav-registry.ts');

    expect(navRegistry).not.toContain("import('./shell/ShellNav')");
    expect(navRegistry).not.toContain('shell: lazy(');
  });

  it('removes the superseded navigation-owned dialog', () => {
    expect(existsSync(resolve(process.cwd(), 'src/app/components/NavPanel/NavSearchDialog.tsx'))).toBe(false);
  });
});
