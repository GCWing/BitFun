import { useEffect, useCallback } from 'react';
import { ChatProvider, useAIInitialization } from '../infrastructure';
import { ViewModeProvider } from '../infrastructure/contexts/ViewModeContext';
import AppLayout from './layout/AppLayout';
import { useCurrentModelConfig } from '../hooks/useModelConfigs';
import { ContextMenuRenderer } from '../shared/context-menu-system/components/ContextMenuRenderer';
import { NotificationContainer, NotificationCenter } from '../shared/notification-system';
import { ConfirmDialogRenderer } from '../component-library';
import { createLogger } from '@/shared/utils/logger';

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
function App() {
  // AI initialization
  const { currentConfig } = useCurrentModelConfig();
  const { isInitialized: aiInitialized, isInitializing: aiInitializing, error: aiError } = useAIInitialization(currentConfig);
  
  // Onboarding state
  const { isOnboardingActive, forceShowOnboarding, completeOnboarding } = useOnboardingStore();
  
  // Handle onboarding completion
  const handleOnboardingComplete = useCallback(() => {
    completeOnboarding();
  }, [completeOnboarding]);

  // Initialize onboarding: check first launch on startup
  useEffect(() => {
    onboardingService.initialize().catch((error) => {
      log.error('Failed to initialize onboarding service', error);
    });
  }, []);

  // In development, trigger onboarding via window.showOnboarding()
  useEffect(() => {
    (window as any).showOnboarding = () => {
      forceShowOnboarding();
      log.debug('Onboarding activated via debug command');
    };
    
    return () => {
      delete (window as any).showOnboarding;
    };
  }, [forceShowOnboarding]);

  // Single-window startup: show main window after React is ready.
  // Key: wait for initial render to avoid showing a blank window.
  useEffect(() => {
    // Note: the main window is created with `visible(false)` in Rust; frontend must call show().
    // Avoid requestAnimationFrame: on some platforms a hidden window pauses rAF,
    // which can keep the UI from ever showing.
    // Use a DOM-ready signal (MutationObserver) to detect initial render,
    // with a bounded fallback timeout to avoid waiting forever.
    let cancelled = false;
    let observer: MutationObserver | null = null;
    let fallbackTimer: number | null = null;

    const showWindow = async (reason: string) => {
      if (cancelled) return;
      cancelled = true;

      if (fallbackTimer !== null) {
        window.clearTimeout(fallbackTimer);
        fallbackTimer = null;
      }
      if (observer) {
        observer.disconnect();
        observer = null;
      }

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
        }
      }
    };

    const isDomReady = () => {
      const root = document.getElementById('root');
      if (!root) return false;

      // This node appears once AppLayout renders (with or without a workspace).
      if (document.querySelector('[data-testid="app-layout"]')) return true;

      // Any first-paint DOM implies the UI is safe to show (e.g. ErrorBoundary screen).
      return root.childElementCount > 0;
    };

    if (isDomReady()) {
      // Microtask: avoid calling show during the initial commit phase.
      Promise.resolve().then(() => {
        void showWindow('dom-ready-immediate');
      });
      return;
    }

    const root = document.getElementById('root');
    if (root) {
      observer = new MutationObserver(() => {
        if (isDomReady()) {
          void showWindow('dom-ready-observed');
        }
      });
      observer.observe(root, { childList: true, subtree: true });
    }

    // Fallback: avoid hanging if observation never fires.
    fallbackTimer = window.setTimeout(() => {
      void showWindow('fallback-timeout');
    }, 2000);

    return () => {
      cancelled = true;
      if (fallbackTimer !== null) {
        window.clearTimeout(fallbackTimer);
      }
      if (observer) {
        observer.disconnect();
      }
    };
  }, []);

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
        await MCPAPI.initializeServersNonDestructive();
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
      <ViewModeProvider defaultMode="agentic">
        <ToolbarModeProvider>
          {/* Onboarding overlay (first launch) */}
          {isOnboardingActive && (
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
        </ToolbarModeProvider>
      </ViewModeProvider>
    </ChatProvider>
  );
}

export default App;
