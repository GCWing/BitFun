import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8');
}

describe('startup performance contract', () => {
  it('keeps editor and tool infrastructure out of the first startup module', () => {
    const source = readSource('../../main.tsx');

    expect(source).not.toMatch(/import\s+['"]monaco-editor\/min\/vs\/editor\/editor\.main\.css['"]/);
    expect(source).not.toMatch(/from\s+['"]@monaco-editor\/react['"]/);
    expect(source).not.toMatch(/from\s+['"]\.\/tools\/initializeTools['"]/);
    expect(source).not.toMatch(/from\s+['"]\.\/shared\/context-menu-system['"]/);

    expect(source).toContain("import('./tools/initializeTools')");
    expect(source).toContain("import('./shared/context-menu-system')");
  });

  it('starts non-critical work after the shell is interactive', () => {
    const source = readSource('../../main.tsx');

    expect(source).toContain("signalName: 'bitfun:interactive-shell-ready'");
    expect(source).not.toContain("signalName: 'bitfun:main-window-shown'");
    expect(source).toContain('fallbackTimeoutMs: 10000');
  });

  it('does not initialize AI from the root app component', () => {
    const source = readSource('../App.tsx');

    expect(source).not.toMatch(/from\s+['"]\.\.\/infrastructure['"]/);
    expect(source).not.toMatch(/useAIInitialization/);
    expect(source).not.toMatch(/useCurrentModelConfig/);
    expect(source).toContain('bitfun:interactive-shell-ready');
  });

  it('loads Monaco styling and loader config only through editor initialization', () => {
    const source = readSource('../../tools/editor/services/MonacoInitManager.ts');

    expect(source).toContain("import('monaco-editor/min/vs/editor/editor.main.css')");
    expect(source).toContain('loader.config');
    expect(source).toContain('MonacoEnvironment');
  });

  it('keeps editor panel implementations lazy from the session shell', () => {
    const source = readSource('../components/panels/base/FlexiblePanel.tsx');
    const componentLibraryBarrel = readSource('../../component-library/components/index.ts');

    expect(source).not.toMatch(/from\s+['"]@\/tools\/editor['"]/);
    expect(source).not.toMatch(/from\s+['"]@\/tools\/git\/components\/GitDiffEditor\/GitDiffEditor['"]/);
    expect(source).toContain("import('@/tools/editor/components/CodeEditor')");
    expect(source).toContain("import('@/tools/editor/components/DiffEditor')");
    expect(source).toContain("import('@/tools/git/components/GitDiffEditor/GitDiffEditor')");
    expect(source).toContain('renderLazyEditor(');
    expect(componentLibraryBarrel).not.toMatch(/CodeEditor/);
  });

  it('keeps theme startup from importing the Monaco runtime', () => {
    const source = readSource('../../infrastructure/theme/integrations/MonacoThemeSync.ts');

    expect(source).not.toMatch(/import\s+\*\s+as\s+monaco\s+from\s+['"]monaco-editor['"]/);
    expect(source).toMatch(/import\s+type\s+\*\s+as\s+Monaco\s+from\s+['"]monaco-editor['"]/);
    expect(source).toContain('attachMonaco');
  });

  it('does not import Monaco runtime from shared edit-target services', () => {
    const source = readSource('../../tools/editor/services/ActiveEditTargetService.ts');

    expect(source).not.toMatch(/import\s+\*\s+as\s+monaco\s+from\s+['"]monaco-editor['"]/);
    expect(source).toMatch(/import\s+type\s+\*\s+as\s+monaco\s+from\s+['"]monaco-editor['"]/);
  });

  it('uses narrow context-menu imports from startup-visible modules', () => {
    const sources = [
      '../../app/scenes/shell/ShellNav.tsx',
      '../../component-library/components/Markdown/Markdown.tsx',
      '../../flow_chat/tool-cards/GenerativeWidgetToolCard.tsx',
      '../../tools/file-system/components/FileSearchResults.tsx',
      '../../tools/generative-widget/useGenerativeWidgetPromptMenu.ts',
      '../../shared/notification-system/providers/NotificationContextMenuProvider.ts',
    ].map(readSource);

    for (const source of sources) {
      expect(source).not.toMatch(/from\s+['"]@\/shared\/context-menu-system['"]/);
    }
  });

  it('avoids the infrastructure barrel from startup-visible modules', () => {
    const sources = [
      '../../flow_chat/components/ChatInput.tsx',
      '../../tools/git/services/GitEventService.ts',
    ].map(readSource);

    for (const source of sources) {
      expect(source).not.toMatch(/from\s+['"]@\/infrastructure['"]/);
      expect(source).not.toMatch(/from\s+['"]\.\.\/\.\.\/\.\.\/infrastructure['"]/);
    }
  });
});
