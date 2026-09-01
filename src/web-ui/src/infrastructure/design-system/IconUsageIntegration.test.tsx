import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import ts from 'typescript';
import { describe, expect, it } from 'vitest';
import { Icon } from '@bitfun/ui';
import { ConfigRefreshButton } from '@/infrastructure/config/components/common';

const sourceRoot = path.resolve(__dirname, '../..');

function source(relativePath: string): string {
  return readFileSync(path.join(sourceRoot, relativePath), 'utf8');
}

function filesIn(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    if (entry.name === 'flow_chat' || entry.name === 'generated') return [];
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) return filesIn(file);
    return /\.tsx$/.test(file) && !/\.(test|appearance)\./.test(file) ? [file] : [];
  });
}

describe('catalog icon consumer integration', () => {
  it('keeps migrated common actions on the shared catalog outside FlowChat', () => {
    const migrated = new Set(['RefreshCw', 'ChevronUp', 'Download', 'Pencil', 'Copy', 'Check', 'SquareTerminal']);
    const leftovers: string[] = [];
    for (const file of filesIn(sourceRoot)) {
      const ast = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true);
      for (const statement of ast.statements) {
        if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)
          || statement.moduleSpecifier.text !== 'lucide-react' || statement.importClause?.isTypeOnly) continue;
        const bindings = statement.importClause?.namedBindings;
        if (!bindings || !ts.isNamedImports(bindings)) continue;
        for (const item of bindings.elements) {
          const name = item.propertyName?.text ?? item.name.text;
          if (!item.isTypeOnly && migrated.has(name)) leftovers.push(`${path.relative(sourceRoot, file)}: ${name}`);
        }
      }
    }
    expect(leftovers).toEqual([]);
  });

  it('uses catalog marks in navigation, the creative entry and string-based menus', () => {
    expect(source('app/components/NavPanel/components/MiniAppEntry.tsx')).toContain('<Icon name="mini-app" size="md"');
    expect(source('app/components/NavBar/NavBar.tsx')).toContain('<Icon name="sidebar-left"');
    expect(source('app/scenes/agents/AgentsScene.tsx')).toContain('<Icon name="creative"');
    expect(source('shared/context-menu-system/components/ContextMenuRenderer.tsx')).toContain("RefreshCw: 'refresh'");
    expect(source('app/components/NavBar/NavBar.tsx')).not.toContain('PanelLeftIcon');
  });

  it('keeps the shared refresh action compact and disables it while loading', () => {
    const props = { tooltip: 'Refresh', onClick: () => {} };
    const idle = renderToStaticMarkup(createElement(ConfigRefreshButton, props));
    expect(idle).toContain('data-bf-name="refresh"');
    expect(idle).toContain('data-size="sm"');
    const loading = renderToStaticMarkup(createElement(ConfigRefreshButton, { ...props, loading: true }));
    expect(loading).toContain('aria-busy="true"');
    expect(loading).toContain('disabled=""');
  });

  it('preserves consumer animation classes and exact non-token dimensions on mask icons', () => {
    const markup = renderToStaticMarkup(createElement(Icon, {
      name: 'refresh', size: 'lg', className: 'browser-panel__spinning',
      style: { width: 20, height: 20 },
    }));
    expect(markup).toContain('browser-panel__spinning');
    expect(markup).toContain('width:20px;height:20px');
    for (const scene of ['BrowserPanel', 'BrowserScene']) {
      const styles = source(`app/scenes/browser/${scene}.scss`);
      expect(styles).toMatch(/&__spinning\s*\{\s*animation:/);
      expect(styles).toMatch(/@media\s*\(prefers-reduced-motion:\s*reduce\)[\s\S]*?__spinning\s*\{\s*animation:\s*none/);
    }
  });
});
