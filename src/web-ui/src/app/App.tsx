import { useEffect, useCallback, useState, useRef } from 'react';
import { ChatProvider, useAIInitialization } from '../infrastructure';
import { ViewModeProvider } from '../infrastructure/contexts/ViewModeContext';
import { SSHRemoteProvider } from '../features/ssh-remote';
import AppLayout from './layout/AppLayout';
import { useCurrentModelConfig } from '../hooks/useModelConfigs';
import { ContextMenuRenderer } from '../shared/context-menu-system/components/ContextMenuRenderer';
import { NotificationContainer, NotificationCenter } from '../shared/notification-system';
import { ConfirmDialogRenderer } from '../component-library';
import { createLogger } from '@/shared/utils/logger';
import { useWorkspaceContext } from '../infrastructure/contexts/WorkspaceContext';
import SplashScreen from './components/SplashScreen/SplashScreen';

// Toolbar Mode
import { ToolbarModeProvider } from '../flow_chat';

// Onboarding
import { OnboardingWizard, useOnboardingStore, onboardingService } from '../features/onboarding';


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
const ENABLE_MAIN_ONBOARDING = false;

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

  // Onboarding state
  const { isOnboardingActive, forceShowOnboarding, completeOnboarding } = useOnboardingStore();
  
  // Handle onboarding completion
  const handleOnboardingComplete = useCallback(() => {
    completeOnboarding();
  }, [completeOnboarding]);

  // Initialize onboarding: check first launch on startup
  useEffect(() => {
    if (!ENABLE_MAIN_ONBOARDING) {
      onboardingService.markCompleted().catch((error) => {
        log.warn('Failed to persist onboarding completion while disabled', error);
      });
      return;
    }

    onboardingService.initialize().catch((error) => {
      log.error('Failed to initialize onboarding service', error);
    });
  }, []);

  // In development, trigger onboarding via window.showOnboarding()
  useEffect(() => {
    if (!ENABLE_MAIN_ONBOARDING) {
      delete (window as any).showOnboarding;
      return;
    }

    (window as any).showOnboarding = () => {
      forceShowOnboarding();
      log.debug('Onboarding activated via debug command');
    };
    
    return () => {
      delete (window as any).showOnboarding;
    };
  }, [forceShowOnboarding]);

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

  // Keep the native window hidden until the startup splash has fully exited.
  // This avoids showing a blank/half-painted webview before the first stable frame.
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
    
    initIdeControl();
    initMCPServers();
    
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

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Escape closes modal
      if (e.key === 'Escape') {
        window.dispatchEvent(new CustomEvent('closePreview'));
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, []);

  // Unified layout via a single AppLayout
  return (
    <ChatProvider>
      <ViewModeProvider defaultMode="coder">
        <SSHRemoteProvider>
          <ToolbarModeProvider>
            {/* Onboarding overlay (first launch) */}
            {ENABLE_MAIN_ONBOARDING && isOnboardingActive && (
              <OnboardingWizard
                onComplete={handleOnboardingComplete}
              />
            )}

            {/* Unified app layout with startup/workspace modes */}
            <AppLayout />

            {/* Context menu renderer */}
            <ContextMenuRenderer />

            {/* Notification system */}
            <NotificationContainer />
            <NotificationCenter />

            {/* Confirm dialog */}
            <ConfirmDialogRenderer />

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
