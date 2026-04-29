import { useEffect, useCallback, useState, useRef } from 'react';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { ChatProvider, useAIInitialization } from '../infrastructure';
import { ViewModeProvider } from '../infrastructure/contexts/ViewModeProvider';
import { SSHRemoteProvider } from '../features/ssh-remote';
import AppLayout from './layout/AppLayout';
import { useCurrentModelConfig } from '../hooks/useModelConfigs';
import { ContextMenuRenderer } from '../shared/context-menu-system/components/ContextMenuRenderer';
import { NotificationContainer, NotificationCenter } from '../shared/notification-system';
import { AnnouncementProvider } from '../shared/announcement-system';
import { ConfirmDialogRenderer } from '../component-library';
import { createLogger } from '@/shared/utils/logger';
import { useWorkspaceContext } from '../infrastructure/contexts/WorkspaceContext';
import SplashScreen from './components/SplashScreen/SplashScreen';
import { useGlobalSceneShortcuts } from './hooks/useGlobalSceneShortcuts';
import { useDebugInspector } from '@/infrastructure/debug/useDebugInspector';

// Toolbar Mode
import { ToolbarModeProvider } from '../flow_chat';

const log = createLogger('App');
/**
 * BitFun main application component.
 *
 * Unified architecture:
 * - Use a single AppLayout component
 * - AppLayout switches content based on workspace presence
 * - Without a workspace: show startup content (branding + actions)
 * - With a workspace: show workspace panels
 * - Header is always present; elements toggle by state
 */
// Minimum time (ms) the splash is shown, so the animation is never a flash.
const MIN_SPLASH_MS = 900;

function App() {
  // AI initialization
  const { currentConfig } = useCurrentModelConfig();
  const { isInitialized: aiInitialized, isInitializing: aiInitializing, error: aiError } = useAIInitialization(currentConfig);

  // Workspace loading state — drives splash exit timing
  const { loading: workspaceLoading } = useWorkspaceContext();

  // Splash screen state
  const [splashVisible, setSplashVisible] = useState(true);
  const [splashExiting, setSplashExiting] = useState(false);
  const mountTimeRef = useRef(Date.now());
  const mainWindowShownRef = useRef(false);

  // Once the workspace finishes loading, wait for the remaining min-display
  // time and then begin the exit animation.
  useEffect(() => {
    if (workspaceLoading) return;
    const elapsed = Date.now() - mountTimeRef.current;
    const remaining = Math.max(0, MIN_SPLASH_MS - elapsed);
    const timer = window.setTimeout(() => setSplashExiting(true), remaining);
    return () => window.clearTimeout(timer);
  }, [workspaceLoading]);

  const handleSplashExited = useCallback(() => {
    setSplashVisible(false);
  }, []);

  const showMainWindow = useCallback(async (reason: string) => {
    if (mainWindowShownRef.current) {
      return;
    }
    mainWindowShownRef.current = true;

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('show_main_window');
      log.debug('Main window shown', { reason });
    } catch (error: any) {
      log.error('Failed to show main window', error);

      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const mainWindow = getCurrentWindow();
        await mainWindow.show();
        await mainWindow.setFocus();
        log.debug('Main window shown via fallback', { reason });
      } catch (fallbackError) {
        log.error('Fallback window show failed', fallbackError);
        mainWindowShownRef.current = false;
      }
    }
  }, []);

  // Reveal the native window as soon as React has painted a frame.
  // The splash still covers the UI, so users see immediate feedback instead
  // of waiting on a hidden window while startup continues in the background.
  useEffect(() => {
    void showMainWindow('startup-overlay');
  }, [showMainWindow]);

  // If the early reveal path fails, keep the old post-splash show as a retry.
  useEffect(() => {
    if (splashVisible) {
      return;
    }

    const timer = window.setTimeout(() => {
      void showMainWindow('startup-complete');
    }, 50);

    return () => window.clearTimeout(timer);
  }, [showMainWindow, splashVisible]);

  // Safety net: if startup gets stuck, reveal the window so the user can see errors.
  useEffect(() => {
    const timer = window.setTimeout(() => {
      void showMainWindow('startup-watchdog');
    }, 10000);

    return () => window.clearTimeout(timer);
  }, [showMainWindow]);

  // Startup logs and initialization
  useEffect(() => {
    log.info('Application started, initializing systems');
    
    // Initialize IDE control system
    const initIdeControl = async () => {
      try {
        const { initializeIdeControl } = await import('../shared/services/ide-control');
        await initializeIdeControl();
        log.debug('IDE control system initialized');
      } catch (error) {
        log.error('Failed to initialize IDE control system', error);
      }
    };
    
    // Initialize MCP servers
    const initMCPServers = async () => {
      try {
        const { MCPAPI } = await import('../infrastructure/api/service-api/MCPAPI');
        await MCPAPI.initializeServers();
        log.debug('MCP servers initialized');
      } catch (error) {
        log.error('Failed to initialize MCP servers', error);
      }
    };

    const initACPClients = async () => {
      try {
        const { ACPClientAPI } = await import('../infrastructure/api/service-api/ACPClientAPI');
        await ACPClientAPI.initializeClients();
        log.debug('ACP clients initialized');
        const requirementProbes = await ACPClientAPI.probeClientRequirements({ force: true });
        log.debug('ACP client requirements probed', { count: requirementProbes.length });
      } catch (error) {
        log.error('Failed to initialize ACP clients', error);
      }
    };

    initIdeControl();
    initMCPServers();
    initACPClients();
    
  }, []);

  // Observe AI initialization state
  useEffect(() => {
    if (aiError) {
      log.error('AI initialization failed', aiError);
    } else if (aiInitialized) {
      log.debug('AI client initialized successfully');
    } else if (!aiInitializing && !currentConfig) {
      log.warn('AI not initialized: waiting for model config');
    } else if (!aiInitializing && currentConfig && !currentConfig.apiKey) {
      log.warn('AI not initialized: missing API key');
    } else if (!aiInitializing && currentConfig && !currentConfig.modelName) {
      log.warn('AI not initialized: missing model name');
    } else if (!aiInitializing && currentConfig && !currentConfig.baseUrl) {
      log.warn('AI not initialized: missing base URL');
    }
  }, [aiInitialized, aiInitializing, aiError, currentConfig]);

  // Block browser-native Ctrl+F (find bar) and Ctrl+R (hard reload).
  // On macOS the equivalent modifiers are Cmd+F / Cmd+R.
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const primary = e.ctrlKey || e.metaKey;
      if (!primary) return;
      const key = e.key.toLowerCase();
      if (key === 'f' || key === 'r') {
        e.preventDefault();
        e.stopPropagation();
      }
    };
    window.addEventListener('keydown', handleKeyDown, { capture: true });
    return () => window.removeEventListener('keydown', handleKeyDown, { capture: true });
  }, []);

  // Escape closes preview overlay (registered via ShortcutManager)
  useShortcut(
    'app.closePreview',
    { key: 'Escape', scope: 'app', allowInInput: true },
    () => window.dispatchEvent(new CustomEvent('closePreview')),
    { priority: 1, description: 'keyboard.shortcuts.app.closePreview' }
  );

  // Top SceneBar: Mod+Alt+1..9 / Mod+Alt+PageUp/PageDown
  useGlobalSceneShortcuts();

  // Debug inspector shortcuts (desktop devtools only)
  useDebugInspector();

  // Unified layout via a single AppLayout
  return (
    <ChatProvider>
      <ViewModeProvider defaultMode="coder">
        <SSHRemoteProvider>
          <ToolbarModeProvider>
            {/* Unified app layout with startup/workspace modes */}
            <AppLayout />

            {/* Context menu renderer */}
            <ContextMenuRenderer />

            {/* Notification system */}
            <NotificationContainer />
            <NotificationCenter />

            {/* Confirm dialog */}
            <ConfirmDialogRenderer />

            {/* Announcement / feature-demo / tips system */}
            <AnnouncementProvider />

            {/* Startup splash — sits above everything, exits once workspace is ready */}
            {splashVisible && (
              <SplashScreen isExiting={splashExiting} onExited={handleSplashExited} />
            )}
          </ToolbarModeProvider>
        </SSHRemoteProvider>
      </ViewModeProvider>
    </ChatProvider>
  );
}

export default App;
