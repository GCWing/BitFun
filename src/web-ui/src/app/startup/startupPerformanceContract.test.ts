import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8');
}

describe('startup performance contract', () => {
  it('keeps the pre-React startup fallback logo-only', () => {
    const source = readSource('../../../index.html');

    expect(source).toContain('<link rel="icon" type="image/png" href="/Logo-ICON-128.png" />');
    expect(source).not.toContain('rel="preload" as="image"');
    expect(source).toContain('class="bitfun-preload__logo"');
    expect(source).toContain('src="/Logo-ICON-128.png"');
    expect(source).toContain('fetchpriority="low"');
    expect(source).not.toContain('Loading workspace...');
    expect(source).not.toContain('bitfun-preload__spinner');
    expect(source).not.toContain('aria-live="polite"');

    expect(source.indexOf('<script type="module" src="/src/main.tsx"></script>')).toBeLessThan(
      source.indexOf('class="bitfun-preload__logo"'),
    );
  });

  it('keeps the startup logo asset transparent without the desktop icon backing plate', async () => {
    const { default: sharp } = await import('sharp');
    const assetPath = fileURLToPath(new URL('../../../public/Logo-ICON-128.png', import.meta.url));
    const { data, info } = await sharp(assetPath).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
    const alphaAt = (x: number, y: number): number => data[(y * info.width + x) * info.channels + 3] ?? 0;

    expect(info.width).toBe(128);
    expect(info.height).toBe(128);
    expect(alphaAt(8, 8)).toBe(0);
    expect(alphaAt(12, 12)).toBe(0);
    expect(alphaAt(20, 20)).toBe(0);
    expect(alphaAt(64, 64)).toBeGreaterThan(240);
  });

  it('keeps editor and tool infrastructure out of the first startup module', () => {
    const source = readSource('../../main.tsx');

    expect(source).not.toMatch(/import\s+['"]monaco-editor\/min\/vs\/editor\/editor\.main\.css['"]/);
    expect(source).not.toMatch(/from\s+['"]@monaco-editor\/react['"]/);
    expect(source).not.toMatch(/from\s+['"]\.\/tools\/initializeTools['"]/);
    expect(source).not.toMatch(/from\s+['"]\.\/shared\/context-menu-system['"]/);

    expect(source).toContain("import('./tools/initializeTools')");
    expect(source).toContain("import('./shared/context-menu-system')");
  });

  it('does not block first React render on frontend log-level config reads', () => {
    const mainSource = readSource('../../main.tsx');
    const loggerSource = readSource('../../shared/utils/logger.ts');
    const themeSource = readSource('../../../../apps/desktop/src/theme.rs');

    expect(mainSource).not.toContain("before_render_step', 'initialize_frontend_log_level_sync'");
    expect(mainSource).not.toContain('before_render_step", "initialize_frontend_log_level_sync"');
    expect(mainSource).toContain('initializeFrontendLogLevelSync');
    expect(mainSource).toContain('installFrontendLogLevelConfigWatcher');
    expect(loggerSource).toContain('__BITFUN_BOOTSTRAP_LOG_LEVEL__');
    expect(themeSource).toContain('__BITFUN_BOOTSTRAP_LOG_LEVEL__');
  });

  it('keeps built-in theme startup on the bootstrap path without pre-render config writes', () => {
    const mainSource = readSource('../../main.tsx');
    const themeServiceSource = readSource('../../infrastructure/theme/core/ThemeService.ts');
    const desktopThemeSource = readSource('../../../../apps/desktop/src/theme.rs');

    expect(desktopThemeSource).toContain('__BITFUN_BOOTSTRAP_THEME_ID__');
    expect(desktopThemeSource).toContain('__BITFUN_BOOTSTRAP_THEME_SELECTION__');
    expect(mainSource).toContain("before_render_step', 'theme_service_initialize'");
    expect(themeServiceSource).toContain('getBootstrapThemeSelection');
    expect(themeServiceSource).toContain('applyThemeSelection(bootstrapSelection, { persist: false })');
    expect(themeServiceSource).toContain('applyThemeSelection(saved, { persist: false })');
    expect(themeServiceSource).toContain('ensureUserThemesLoaded');
    expect(mainSource).toContain('themeService.ensureUserThemesLoaded()');
    expect(mainSource.indexOf('themeService.ensureUserThemesLoaded()')).toBeGreaterThan(
      mainSource.indexOf('async function initializeAfterRender()'),
    );
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
    expect(source).not.toMatch(/from\s+['"]@\/infrastructure\/config\/services\/AIExperienceConfigService['"]/);
    expect(source).toContain('bitfun:interactive-shell-ready');
  });

  it('keeps the heavy app layout out of the root startup module', () => {
    const source = readSource('../App.tsx');

    expect(source).not.toMatch(/import\s+AppLayout\s+from\s+['"]\.\/layout\/AppLayout['"]/);
    expect(source).toContain("import('./layout/AppLayout')");
    expect(source).toContain('app_layout_ready');
    expect(source).toContain('!appLayoutReady');
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

  it('prewarms editor runtime only after the shell is interactive', () => {
    const source = readSource('../App.tsx');

    expect(source).toContain('interactiveShellReady');
    expect(source).toContain("import('@/tools/editor/services/MonacoStartupWarmup')");
    expect(source).toContain('scheduleMonacoStartupWarmup()');
  });

  it('does not remount the historical message list when full hydration prepends older turns', () => {
    const source = readSource('../../flow_chat/components/modern/VirtualMessageList.tsx');

    expect(source).not.toContain('firstVirtualItemTurnId');
    expect(source).not.toMatch(/key=\{`\$\{activeSession\?\.sessionId[^`]+firstVirtualItemTurnId/);
  });

  it('keeps read-only thread goal access metadata-only for unloaded sessions', () => {
    const source = readSource('../../../../apps/desktop/src/api/agentic_api.rs');
    const getStart = source.indexOf('pub async fn get_session_thread_goal');
    const clearStart = source.indexOf('pub async fn clear_session_thread_goal');
    const getSource = source.slice(getStart, clearStart);

    expect(getStart).toBeGreaterThan(-1);
    expect(clearStart).toBeGreaterThan(getStart);
    expect(getSource).toContain('resolve_session_workspace_path_for_thread_goal_read');
    expect(getSource).not.toContain('ensure_session_for_thread_goal');
    expect(getSource).not.toContain('restore_session');
  });

  it('keeps Git diff editor from importing the broad editor barrel', () => {
    const source = readSource('../../tools/git/components/GitDiffEditor/GitDiffEditor.tsx');

    expect(source).not.toMatch(/from\s+['"]@\/tools\/editor['"]/);
    expect(source).toContain("from '@/tools/editor/components/DiffEditor'");
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
      '../../app/layout/AppLayout.tsx',
      '../../flow_chat/components/ChatInput.tsx',
      '../../tools/git/services/GitEventService.ts',
    ].map(readSource);

    for (const source of sources) {
      expect(source).not.toMatch(/from\s+['"]@\/infrastructure['"]/);
      expect(source).not.toMatch(/from\s+['"]\.\.\/\.\.\/\.\.\/infrastructure['"]/);
    }

    expect(sources[0]).not.toMatch(/from\s+['"]@\/infrastructure\/config['"]/);
    expect(sources[0]).toContain("from '@/infrastructure/config/services/ConfigManager'");
  });

  it('avoids the broad flow-chat barrel from startup-visible shell modules', () => {
    const sources = [
      '../App.tsx',
      '../../app/layout/AppLayout.tsx',
    ].map(readSource);

    for (const source of sources) {
      expect(source).not.toMatch(/from\s+['"]\.\.\/flow_chat['"]/);
      expect(source).not.toMatch(/from\s+['"]\.\.\/\.\.\/flow_chat['"]/);
      expect(source).not.toMatch(/from\s+['"]@\/flow_chat['"]/);
    }
  });

  it('keeps Agent companion implementation modules out of the root startup bundle', () => {
    const source = readSource('../App.tsx');

    expect(source).not.toMatch(/from\s+['"]@\/flow_chat\/utils\/agentCompanionActivity['"]/);
    expect(source).not.toMatch(/from\s+['"]@\/flow_chat\/services\/AgentCompanionActivityBridge['"]/);
    expect(source).not.toMatch(/from\s+['"]\.\/services\/openAgentCompanionSession['"]/);
    expect(source).not.toMatch(/from\s+['"]@\/infrastructure\/config\/services\/AgentCompanionWindowService['"]/);
    expect(source).toContain("import('@/flow_chat/utils/agentCompanionActivity')");
    expect(source).toContain("import('@/flow_chat/services/AgentCompanionActivityBridge')");
    expect(source).toContain("import('./services/openAgentCompanionSession')");
  });
});
