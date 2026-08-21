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
