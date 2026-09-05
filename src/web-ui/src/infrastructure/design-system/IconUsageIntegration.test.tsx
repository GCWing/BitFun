import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import ts from 'typescript';
import { describe, expect, it } from 'vitest';
import { Icon } from '@openbitfun/ui';
import { ConfigRefreshButton } from '@/infrastructure/config/components/common';

const sourceRoot = path.resolve(__dirname, '../..');

function source(relativePath: string): string {
  return readFileSync(path.join(sourceRoot, relativePath), 'utf8');
}

function filesIn(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    if (entry.name === 'generated') return [];
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) return filesIn(file);
    return /\.tsx?$/.test(file) && !/\.(test|appearance)\./.test(file) ? [file] : [];
  });
}

describe('catalog icon consumer integration', () => {
  it('keeps every migrated legacy icon on the shared catalog', () => {
    const migrated = new Set([
      'AppWindow',
      'ArrowDownToLine',
      'ArrowUp',
      'BadgeCheck',
      'Check',
      'CheckCircle',
      'CheckCircle2',
      'ChevronDown',
      'ChevronLeft',
      'ChevronRight',
      'ChevronUp',
      'Circle',
      'Clock',
      'Clock3',
      'ClipboardCopy',
      'Copy',
      'Download',
      'Edit',
      'Edit3',
      'ExternalLink',
      'Eye',
      'Files',
      'FileDown',
      'FileEdit',
      'FileImage',
      'FileInput',
      'FileOutput',
      'FilePenLine',
      'Folder',
      'GitBranch',
      'GitCommitHorizontal',
      'Globe',
      'Globe2',
      'Image',
      'Info',
      'Link',
      'Link2',
      'ListFilter',
      'MessageSquarePlus',
      'Mic',
      'MoreHorizontal',
      'Pencil',
      'PictureInPicture2',
      'Paintbrush',
      'PanelRightOpen',
      'Plus',
      'Puzzle',
      'RefreshCw',
      'Search',
      'Settings',
      'Settings2',
      'SlidersHorizontal',
      'Sparkle',
      'Sparkles',
      'SquareTerminal',
      'Terminal',
      'Trash2',
      'User',
      'WandSparkles',
      'X',
    ]);
    const exemptions = new Set([
      'flow_chat/components/ChatInputWorkspaceStrip.tsx: Circle',
      'flow_chat/components/ReasoningPresetSelector.tsx: Circle',
    ]);
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
          const location = `${path.relative(sourceRoot, file).replaceAll('\\', '/')}: ${name}`;
          if (!item.isTypeOnly && migrated.has(name) && !exemptions.has(location)) {
            leftovers.push(location);
          }
        }
      }
    }
    expect(leftovers).toEqual([]);
  });

  it('uses catalog marks in navigation, the creative entry and string-based menus', () => {
    expect(source('app/components/NavPanel/components/MiniAppEntry.tsx')).toContain('<Icon name="mini-app" size="md"');
    expect(source('app/components/NavBar/NavBar.tsx')).toContain('<Icon name="sidebar-left"');
    const harnessSource = source('app/scenes/agents/components/AgentHarnessOverview.tsx');
    expect(harnessSource).toContain("{ id: 'creative', icon: 'creative'");
    expect(harnessSource).toContain('<Icon name={icon}');
    expect(source('shared/context-menu-system/components/ContextMenuRenderer.tsx')).toContain("RefreshCw: 'refresh'");
    expect(source('shared/context-menu-system/components/ContextMenuRenderer.tsx')).toContain("MessageSquarePlus: 'side-chat'");
    expect(source('shared/context-menu-system/components/ContextMenuRenderer.tsx')).toContain("FileInput: 'duplicate'");
    expect(source('shared/context-menu-system/components/ContextMenuRenderer.tsx')).toContain("FileOutput: 'duplicate'");
    expect(source('shared/context-menu-system/components/ContextMenuRenderer.tsx')).toContain("PanelRightOpen: 'browser'");
    expect(source('app/components/NavBar/NavBar.tsx')).not.toContain('PanelLeftIcon');
  });

  it('uses canonical catalog names instead of compatibility aliases', () => {
    const leftovers = filesIn(sourceRoot).flatMap(file => {
      const contents = readFileSync(file, 'utf8');
      return ['download', 'circle']
        .filter(name => contents.includes(`name="${name}"`))
        .map(name => `${path.relative(sourceRoot, file).replaceAll('\\', '/')}: ${name}`);
    });
    expect(leftovers).toEqual([]);
  });

  it('keeps the shared refresh action compact and disables it while loading', () => {
    const props = { tooltip: 'Refresh', onClick: () => {} };
    const idle = renderToStaticMarkup(createElement(ConfigRefreshButton, props));
    expect(idle).toContain('data-openbitfun-name="refresh"');
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
