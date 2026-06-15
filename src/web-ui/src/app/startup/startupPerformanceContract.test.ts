import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

import { shouldScheduleDeferredStartupSystems } from './deferredStartupGate';
import { STARTUP_OVERLAY_HIDDEN_EVENT } from './startupSignals';

function readSource(relativePath: string): string {
  return readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8');
}

function dynamicImportSpecifiers(source: string): string[] {
  return Array.from(
    source.matchAll(/import\(\s*(['"])(.*?)\1\s*\)/g),
    match => match[2]
  );
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

  it('keeps the required i18n provider off an async startup waterfall', () => {
    const source = readSource('../../main.tsx');

    expect(source).toContain(
      'import { I18nProvider } from "./infrastructure/i18n/providers/I18nProvider"'
    );
    expect(source).not.toMatch(/import\(['"]\.\/infrastructure\/i18n['"]\)/);
    expect(source).toContain("step: 'load_i18n_provider'");
    expect(source).toContain("mode: 'static'");
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

  it('keeps startup keybindings on the bootstrap path instead of a first-window IPC', () => {
    const configManagerSource = readSource('../../infrastructure/config/services/ConfigManager.ts');
    const themeSource = readSource('../../../../apps/desktop/src/theme.rs');

    expect(themeSource).toContain('__BITFUN_BOOTSTRAP_KEYBINDINGS__');
    expect(themeSource).toContain('keybindings: global_config.app.keybindings');
    expect(themeSource).toContain('MAX_BOOTSTRAP_KEYBINDINGS_JSON_BYTES');
    expect(themeSource).toContain('.filter(|json| json.len() <= MAX_BOOTSTRAP_KEYBINDINGS_JSON_BYTES)');
    expect(configManagerSource).toContain('consumeBootstrapOptionalConfig');
    expect(configManagerSource).toContain('__BITFUN_BOOTSTRAP_KEYBINDINGS__');
    expect(configManagerSource).toContain("path !== 'app.keybindings'");
    expect(configManagerSource).toContain('delete globalThis.__BITFUN_BOOTSTRAP_KEYBINDINGS__');
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

  it('keeps Windows startup show wait state-driven instead of fixed-delay only', () => {
    const desktopThemeSource = readSource('../../../../apps/desktop/src/theme.rs');

    expect(desktopThemeSource).toContain('WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_MAX');
    expect(desktopThemeSource).toContain('WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_MIN');
    expect(desktopThemeSource).toContain('WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_POLL');
    expect(desktopThemeSource).toContain('windows_maximize_show_wait_action');
    expect(desktopThemeSource).toContain('window.is_maximized()');
    expect(desktopThemeSource).toContain('windows_show_after_maximize_wait');
    expect(desktopThemeSource).not.toContain(
      'std::thread::sleep(std::time::Duration::from_millis(150))'
    );
  });

  it('starts non-critical work after the startup overlay handoff', () => {
    const source = readSource('../../main.tsx');

    expect(STARTUP_OVERLAY_HIDDEN_EVENT).toBe('bitfun:startup-overlay-hidden');
    expect(source).toContain('STARTUP_OVERLAY_HIDDEN_EVENT');
    expect(source).not.toContain("signalName: 'bitfun:interactive-shell-ready'");
    expect(source).not.toContain("signalName: 'bitfun:main-window-shown'");
    expect(source).toContain('fallbackTimeoutMs: 10000');
  });

  it('starts deferred app systems only after the startup overlay has handed off', () => {
    const source = readSource('../App.tsx');

    expect(shouldScheduleDeferredStartupSystems({
      interactiveShellReady: true,
      startupOverlayVisible: true,
    })).toBe(false);
    expect(shouldScheduleDeferredStartupSystems({
      interactiveShellReady: true,
      startupOverlayVisible: false,
    })).toBe(true);
    expect(source).toContain('shouldScheduleDeferredStartupSystems({ interactiveShellReady, startupOverlayVisible })');
    expect(source).toContain('window.dispatchEvent(new CustomEvent(STARTUP_OVERLAY_HIDDEN_EVENT))');
    expect(source).toContain('}, [interactiveShellReady, startupOverlayVisible]);');
  });

  it('does not initialize AI from the root app component', () => {
    const source = readSource('../App.tsx');

    expect(source).not.toMatch(/from\s+['"]\.\.\/infrastructure['"]/);
    expect(source).not.toMatch(/useAIInitialization/);
    expect(source).not.toMatch(/useCurrentModelConfig/);
    expect(source).not.toMatch(/from\s+['"]@\/infrastructure\/config\/services\/AIExperienceConfigService['"]/);
    expect(source).toContain('bitfun:interactive-shell-ready');
    expect(source).toContain('STARTUP_OVERLAY_HIDDEN_EVENT');
  });

  it('keeps the heavy app layout out of the root startup module', () => {
    const source = readSource('../App.tsx');

    expect(source).not.toMatch(/import\s+AppLayout\s+from\s+['"]\.\/layout\/AppLayout['"]/);
    expect(source).toContain("import('./layout/AppLayout')");
    expect(source).toContain('app_layout_ready');
    expect(source).toContain('!appLayoutReady');
  });

  it('releases interactive shell readiness without waiting for an extra AppLayout state commit', () => {
    const source = readSource('../App.tsx');

    expect(source).toContain('workspaceLoadingRef.current');
    expect(source).toContain('appLayoutReadyRef.current');
    expect(source).toContain('interactiveShellReadyFrameRef.current');
    expect(source).toContain('markInteractiveShellReadyIfReady');
    expect(source).toContain('releaseInteractiveShellReadyIfReady');
    expect(source).toContain('useLayoutEffect');
    expect(source).toContain('requestAnimationFrame');
    expect(source).toContain('interactive_shell_ready_after_paint_scheduled');
    expect(source).toContain("markInteractiveShellReadyIfReady('app-layout-ready')");
    expect(source).toContain("markInteractiveShellReadyIfReady('workspace-or-layout-state')");
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

  it('keeps terminal xterm runtime out of session startup until terminal output is rendered', () => {
    const sessionSceneSource = readSource('../scenes/session/SessionScene.tsx');
    const flexiblePanelSource = readSource('../components/panels/base/FlexiblePanel.tsx');
    const terminalToolCardSource = readSource('../../flow_chat/tool-cards/TerminalToolCard.tsx');
    const execProcessToolCardSource = readSource('../../flow_chat/tool-cards/ExecProcessToolCardView.tsx');
    const backgroundCommandOutputPanelSource = readSource(
      '../../flow_chat/components/background-command/BackgroundCommandOutputPanel.tsx'
    );
    const lazyTerminalOutputSource = readSource('../../tools/terminal/components/LazyTerminalOutputRenderer.tsx');

    expect(sessionSceneSource).not.toMatch(/from\s+['"]@\/tools\/terminal['"]/);
    expect(sessionSceneSource).toContain(
      "from '@/tools/terminal/services/terminalPanelPreferenceService'"
    );
    expect(flexiblePanelSource).not.toContain("import('@/tools/terminal')");
    expect(flexiblePanelSource).toContain(
      "import('@/tools/terminal/components/ConnectedTerminal')"
    );
    expect(terminalToolCardSource).not.toMatch(/from\s+['"]@\/tools\/terminal\/components['"]/);
    expect(terminalToolCardSource).toContain(
      "from '@/tools/terminal/components/LazyTerminalOutputRenderer'"
    );
    expect(execProcessToolCardSource).toContain(
      "from '@/tools/terminal/components/LazyTerminalOutputRenderer'"
    );
    expect(backgroundCommandOutputPanelSource).not.toMatch(/from\s+['"]@\/tools\/terminal\/components['"]/);
    expect(backgroundCommandOutputPanelSource).toContain(
      "from '@/tools/terminal/components/LazyTerminalOutputRenderer'"
    );
    expect(lazyTerminalOutputSource).toContain("import('./TerminalOutputRenderer')");
  });

  it('keeps settings config panels lazy by active tab', () => {
    const source = readSource('../scenes/settings/SettingsScene.tsx');

    expect(source).not.toMatch(/import\s+AIModelConfig\s+from/);
    expect(source).not.toMatch(/import\s+McpToolsConfig\s+from/);
    expect(source).not.toMatch(/import\s+AcpAgentsConfig\s+from/);
    expect(source).not.toMatch(/import\s+EditorConfig\s+from/);
    expect(source).not.toMatch(/import\s+BasicsConfig\s+from/);
    expect(source).not.toMatch(/import\s+AppearanceConfig\s+from/);
    expect(source).not.toMatch(/import\s+ReviewConfig\s+from/);
    expect(source).not.toMatch(/import\s+QuickActionsConfig\s+from/);
    expect(source).toContain("lazy(() => import('../../../infrastructure/config/components/AIModelConfig'))");
    expect(source).toContain("lazy(() => import('../../../infrastructure/config/components/BasicsConfig'))");
    expect(source).toContain("lazy(() => import('./components/ArchivedSessionsConfig'))");
    expect(source).toContain('<Suspense');
  });

  it('keeps tool-card metadata separate from heavy card implementations', () => {
    const registrySource = readSource('../../flow_chat/tool-cards/index.ts');
    const metadataSource = readSource('../../flow_chat/tool-cards/toolCardMetadata.ts');
    const flowToolCardSource = readSource('../../flow_chat/components/FlowToolCard.tsx');
    const modelRoundItemSource = readSource('../../flow_chat/components/modern/ModelRoundItem.tsx');
    const flowStoreSource = readSource('../../flow_chat/store/modernFlowChatStore.ts');
    const componentRegistrySource = readSource('../../component-library/components/registry.tsx');
    const keyboardShortcutsSource = readSource('../scenes/settings/components/KeyboardShortcutsTab.tsx');

    expect(metadataSource).toContain('TOOL_CARD_CONFIGS');
    expect(metadataSource).toContain('isCollapsibleTool');
    expect(metadataSource).not.toMatch(/from\s+['"]\.\/TerminalToolCard['"]/);
    expect(metadataSource).not.toMatch(/from\s+['"]\.\/FileOperationToolCard['"]/);

    expect(registrySource).not.toContain('export const TOOL_CARD_CONFIGS');
    expect(registrySource).toContain("from './toolCardMetadata'");
    expect(flowToolCardSource).toContain("from '../tool-cards/toolCardMetadata'");
    expect(flowToolCardSource).toContain("from '../tool-cards'");
    expect(modelRoundItemSource).toContain("from '../../tool-cards/toolCardMetadata'");
    expect(modelRoundItemSource).not.toMatch(/from\s+['"]\.\.\/\.\.\/tool-cards['"]/);
    expect(flowStoreSource).toContain("from '../tool-cards/toolCardMetadata'");
    expect(flowStoreSource).not.toMatch(/from\s+['"]\.\.\/tool-cards['"]/);
    expect(componentRegistrySource).toContain("from '@/flow_chat/tool-cards/toolCardMetadata'");
    expect(keyboardShortcutsSource).not.toMatch(/from\s+['"]@\/infrastructure\/config['"]/);
    expect(keyboardShortcutsSource).toContain(
      "from '@/infrastructure/config/services/ConfigManager'"
    );
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

  it('prewarms editor runtime only after the shell is interactive and visible', () => {
    const source = readSource('../App.tsx');

    expect(source).toContain('interactiveShellReady');
    expect(source).toContain('startupOverlayVisible');
    expect(source).toContain("import('@/tools/editor/services/MonacoStartupWarmup')");
    expect(source).toContain('scheduleMonacoStartupWarmup()');
  });

  it('does not let startup visibility retries reopen a user-hidden main window', () => {
    const source = readSource('../App.tsx');

    expect(source).toContain('userCloseRequestedRef');
    expect(source).toContain("listen('bitfun_main_window_close_requested'");
    expect(source).toContain('user-close-requested');
    expect(source).toContain('startup-complete');
    expect(source).toContain('startup-watchdog');
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

  it('defers passive historical thread-goal refresh without delaying explicit goal entry', () => {
    const source = readSource('../../flow_chat/hooks/useThreadGoalController.ts');
    const passiveRefreshEffectStart = source.indexOf('if (session?.isHistorical)');
    const openGoalEntryStart = source.indexOf('const openGoalEntry = useCallback');

    expect(passiveRefreshEffectStart).toBeGreaterThan(-1);
    expect(source).toContain('HISTORICAL_THREAD_GOAL_REFRESH_DELAY_MS');
    expect(source).toContain('globalThis.setTimeout(() => {');
    expect(source).toContain('globalThis.clearTimeout(timeoutId)');
    expect(openGoalEntryStart).toBeGreaterThan(passiveRefreshEffectStart);
    expect(source.slice(openGoalEntryStart)).toContain('await fetchSessionThreadGoal(session)');
  });

  it('uses the history open intent as a strict before-hydrate activation gate without adding a second paint wait', () => {
    const source = readSource('../../app/components/NavPanel/sections/sessions/SessionsSection.tsx');
    const sessionModuleSource = readSource('../../flow_chat/services/flow-chat-manager/SessionModule.ts');
    const intentSource = readSource('../../flow_chat/services/sessionOpenIntent.ts');
    const dispatchResultStart = source.indexOf("type HistoryOpenIntentDispatchResult = 'none' | 'dispatched' | 'already-pending'");
    const duplicateIntentStart = source.indexOf("return 'already-pending'");
    const switchStart = source.indexOf('const handleSwitch = useCallback');
    const pointerDownStart = source.indexOf('const handleSessionOpenPointerDown = useCallback');
    const beforeHydrateStart = sessionModuleSource.indexOf("activation: 'before-hydrate'");
    const hydrateBeforeSwitchStart = sessionModuleSource.indexOf('if (shouldHydrateBeforeSwitch)');

    expect(dispatchResultStart).toBeGreaterThan(-1);
    expect(duplicateIntentStart).toBeGreaterThan(dispatchResultStart);
    expect(source).toContain("historyOpenIntentDispatch !== 'none'");
    expect(source).not.toContain('if (historyOpenIntentDispatched)');
    expect(pointerDownStart).toBeGreaterThan(switchStart);
    expect(source.slice(pointerDownStart)).toContain('dispatchHistoryOpenIntentForSession(session)');
    expect(intentSource).toContain('RECENT_HISTORY_OPEN_INTENT_MS');
    expect(intentSource).toContain('HISTORY_SESSION_OPEN_TRANSITION_MAX_MS');
    expect(intentSource).toContain('subscribeHistorySessionOpenTransition');
    expect(sessionModuleSource).toContain('consumeRecentHistorySessionOpenIntent(sessionId)');
    expect(beforeHydrateStart).toBeGreaterThan(-1);
    expect(hydrateBeforeSwitchStart).toBeGreaterThan(-1);
    expect(beforeHydrateStart).toBeLessThan(hydrateBeforeSwitchStart);
    expect(sessionModuleSource).toContain("shouldHydrateBeforeSwitch ? 'after-hydrate' : 'immediate'");
  });

  it('keeps passive chat Git refresh out of the history open transition window', () => {
    const chatInputSource = readSource('../../flow_chat/components/ChatInput.tsx');
    const fileCardSource = readSource('../../flow_chat/tool-cards/FileOperationToolCard.tsx');
    const workspaceItemSource = readSource('../../app/components/NavPanel/sections/workspaces/WorkspaceItem.tsx');

    expect(chatInputSource).toContain('useSyncExternalStore');
    expect(chatInputSource).toContain('getHistorySessionOpenTransitionSnapshot');
    expect(chatInputSource).toContain('deferChatStripPassiveGitRefresh');
    expect(chatInputSource).toContain('historySessionOpenTransition !== null');
    expect(fileCardSource).toContain('getHistorySessionOpenTransitionSnapshot');
    expect(fileCardSource).toContain('historySessionOpenTransition === null');
    expect(fileCardSource).toContain("displayContext !== 'subagent-projection'");
    expect(workspaceItemSource).toContain('getHistorySessionOpenTransitionSnapshot');
    expect(workspaceItemSource).toContain('suppressWorkspaceGitRefreshOnMountDuringSessionTransition');
    expect(workspaceItemSource).toContain('subscribeHistorySessionOpenTransition');
    expect(workspaceItemSource).toContain('WORKSPACE_GIT_PENDING_CANCEL_SOURCES');
    expect(workspaceItemSource).toContain('cancelPendingRefresh');
    expect(workspaceItemSource).toContain('historySessionOpenTransition !== null');
  });

  it('keeps non-active workspace session metadata out of the first startup window', () => {
    const source = readSource('../../app/components/NavPanel/sections/sessions/SessionsSection.tsx');

    expect(source).toContain('isActiveWorkspace = true');
    expect(source).not.toContain('isActiveWorkspace: _isActiveWorkspace');
    expect(source).toContain('getInitialSessionMetadataLoadMode');
    expect(source).toContain("loadMode === 'immediate'");
    expect(source).toContain("loadMode === 'after-startup-paint'");
    expect(source).toContain('scheduleAfterStartupSignal');
    expect(source).toContain('scheduleAfterStartupPaint');
    expect(source).toContain('hasStartupOverlayHandedOff');
    expect(source).toContain('SESSION_METADATA_DEFERRED_SIGNAL');
    expect(source).toContain('sessions_nav_initial_active');
    expect(source).toContain('sessions_nav_initial_deferred');
    expect(source).toContain('data-session-nav-toggle-action');
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

  it('keeps markdown content rendering off the components i18n subscription path', () => {
    const source = readSource('../../component-library/components/Markdown/Markdown.tsx');
    const mathSource = readSource('../../component-library/components/Markdown/MarkdownMathRenderer.tsx');

    expect(source).not.toContain("useI18n('components')");
    expect(source).not.toContain('useI18n("components")');
    expect(source).toContain("import { i18nService } from '@/infrastructure/i18n'");
    expect(source).not.toContain("from 'remark-math'");
    expect(source).not.toContain("from 'rehype-katex'");
    expect(source).not.toContain("import 'katex/dist/katex.min.css'");
    expect(source).toContain("import('./MarkdownMathRenderer')");
    expect(mathSource).toContain("from 'remark-math'");
    expect(mathSource).toContain("from 'rehype-katex'");
    expect(mathSource).toContain("import 'katex/dist/katex.min.css'");
  });

  it('avoids the infrastructure barrel from startup-visible modules', () => {
    const sources = [
      '../../app/layout/AppLayout.tsx',
      '../../app/hooks/useDialogCompletionNotify.ts',
      '../../infrastructure/update/DailyAppUpdateGate.tsx',
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

  it('keeps on-demand workspace utility dialogs out of startup-visible chunks', () => {
    const appLayoutSource = readSource('../../app/layout/AppLayout.tsx');
    const workspaceItemSource = readSource('../../app/components/NavPanel/sections/workspaces/WorkspaceItem.tsx');
    const sessionsSectionSource = readSource('../../app/components/NavPanel/sections/sessions/SessionsSection.tsx');
    const footerActionsSource = readSource('../../app/components/NavPanel/components/PersistentFooterActions.tsx');
    const newProjectDialogSource = readSource('../../app/components/NewProjectDialog/NewProjectDialog.tsx');
    const relatedPathsDialogSource = readSource(
      '../../app/components/NavPanel/sections/workspaces/WorkspaceRelatedPathsDialog.tsx'
    );

    expect(appLayoutSource).not.toMatch(/import\s+\{\s*open\s*\}\s+from\s+['"]@tauri-apps\/plugin-dialog['"]/);
    expect(appLayoutSource).not.toMatch(/import\s+\{\s*NewProjectDialog\s*\}\s+from/);
    expect(appLayoutSource).toContain('const NewProjectDialog = lazy');
    expect(appLayoutSource).toContain("import('../components/NewProjectDialog')");
    expect(appLayoutSource).toContain('{showNewProjectDialog && (');

    expect(workspaceItemSource).not.toMatch(/import\s+WorkspaceRelatedPathsDialog\s+from/);
    expect(workspaceItemSource).not.toMatch(/import\s+WorkspaceSessionBatchModal\s+from/);
    expect(workspaceItemSource).not.toMatch(/import\s+ScheduledJobsModal\s+from/);
    expect(workspaceItemSource).toContain("lazy(() => import('./WorkspaceRelatedPathsDialog'))");
    expect(workspaceItemSource).toContain("lazy(() => import('./WorkspaceSessionBatchModal'))");
    expect(workspaceItemSource).toContain("lazy(() => import('@/app/components/scheduled-jobs/ScheduledJobsModal'))");
    expect(workspaceItemSource).toContain('{relatedPathsDialogOpen && (');
    expect(workspaceItemSource).toContain('{sessionBatchModalOpen && (');
    expect(workspaceItemSource).toContain('{scheduledJobsModalOpen && (');

    expect(sessionsSectionSource).not.toMatch(/import\s+ScheduledJobsModal\s+from/);
    expect(sessionsSectionSource).toContain("lazy(() => import('@/app/components/scheduled-jobs/ScheduledJobsModal'))");
    expect(sessionsSectionSource).toContain('{scheduledJobsSession && (');

    expect(footerActionsSource).not.toMatch(/import\s+\{\s*RemoteConnectDialog\s*\}\s+from/);
    expect(footerActionsSource).toContain("lazy(() => import('../../RemoteConnectDialog'))");
    expect(footerActionsSource).toContain('{showRemoteConnect && (');

    expect(newProjectDialogSource).not.toMatch(/from\s+['"]@tauri-apps\/plugin-dialog['"]/);
    expect(newProjectDialogSource).toContain("await import('@tauri-apps/plugin-dialog')");
    expect(relatedPathsDialogSource).not.toMatch(/from\s+['"]@tauri-apps\/plugin-dialog['"]/);
    expect(relatedPathsDialogSource).toContain("await import('@tauri-apps/plugin-dialog')");
  });

  it('keeps startup session metadata paging on the narrow SessionAPI entrypoint', () => {
    const source = readSource('../../flow_chat/store/FlowChatStore.ts');
    const imports = dynamicImportSpecifiers(source);

    expect(source).toContain("import('@/infrastructure/api/service-api/SessionAPI')");
    expect(imports).not.toContain('@/infrastructure/api');
  });

  it('keeps historical session restore on the narrow AgentAPI entrypoint', () => {
    const source = readSource('../../flow_chat/store/FlowChatStore.ts');
    const imports = dynamicImportSpecifiers(source);
    const agentApiDynamicImports = imports.filter(specifier => specifier.endsWith('/AgentAPI'));

    expect(agentApiDynamicImports.length).toBeGreaterThan(0);
    expect(new Set(agentApiDynamicImports)).toEqual(
      new Set(['@/infrastructure/api/service-api/AgentAPI'])
    );
    expect(imports).not.toContain('@/infrastructure/api');
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
