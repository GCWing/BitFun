/**
 * Main application layout.
 * Overall layout: Header + workspace content + bottom bar.
 *
 * Unified layout:
 * - Without a workspace: show startup content (branding + actions)
 * - With a workspace: show workspace panels
 * - Header is always present; elements toggle by state
 */

import React, { useState, useCallback } from 'react';
import { useWorkspaceContext } from '../../infrastructure/contexts/WorkspaceContext';
import { useWindowControls } from '../hooks/useWindowControls';
import { useApp } from '../hooks/useApp';
import { useViewMode } from '../../infrastructure/contexts/ViewModeContext';
import { FlowChatManager } from '../../flow_chat/services/FlowChatManager';
import WorkspaceLayout from './WorkspaceLayout';
import AppBottomBar from '../components/BottomBar/AppBottomBar';
import Header from '../components/Header/Header';
import { StartupContent } from '../components/StartupContent';
import { ChatInput, ToolbarMode, useToolbarModeContext } from '../../flow_chat';
import { appManager } from '../';
import { createLogger } from '@/shared/utils/logger';
import { useI18n } from '@/infrastructure/i18n';
import './AppLayout.scss';

const log = createLogger('AppLayout');

interface AppLayoutProps {
  /** Transition class name */
  className?: string;
}

const AppLayout: React.FC<AppLayoutProps> = ({
  className = '',
}) => {
  const { t } = useI18n('components');
  // Workspace state
  const { currentWorkspace, hasWorkspace, openWorkspace } = useWorkspaceContext();
  
  // View mode (agentic/editor)
  const { isAgenticMode } = useViewMode();
  
  // Toolbar mode
  const { isToolbarMode } = useToolbarModeContext();

  // Window controls
  const { handleMinimize, handleMaximize, handleClose, handleHomeClick, isMaximized } =
    useWindowControls({ isToolbarMode });
  
  // Application state
  const { state, toggleLeftPanel, toggleRightPanel, switchLeftPanelTab } = useApp();
  
  // Transition state: startup content to workspace
  const [isTransitioning, setIsTransitioning] = useState(false);
  // Header sweep effect state
  const [isSweepGlowing, setIsSweepGlowing] = useState(false);
  
  // Handle workspace selection
  const handleWorkspaceSelected = useCallback(async (workspacePath: string, projectDescription?: string) => {
    try {
      log.info('Workspace selected', { workspacePath });
      
      // Persist project description if provided
      if (projectDescription && projectDescription.trim()) {
        sessionStorage.setItem('pendingProjectDescription', projectDescription.trim());
      }
      
      // Start transition
      setIsTransitioning(true);
      
      // Open workspace
      const workspace = await openWorkspace(workspacePath);
      
      // Configure layout based on view mode.
      // Agentic mode: collapse right panel; Editor mode: expand.
      appManager.updateLayout({
        leftPanelCollapsed: false,
        rightPanelCollapsed: isAgenticMode
      });
      
      // Trigger header sweep effect immediately
      setIsSweepGlowing(true);
      // Stop sweep after 1.2s (0.8s animation + buffer)
      setTimeout(() => {
        setIsSweepGlowing(false);
      }, 1200);
      
      // Transition complete
      setTimeout(() => {
        setIsTransitioning(false);
      }, 600);
      
    } catch (error) {
      log.error('Failed to open workspace', error);
      setIsTransitioning(false);
      
      // Show error notification
      import('@/shared/notification-system').then(({ notificationService }) => {
        const errorMessage = error instanceof Error ? error.message : String(error);
        notificationService.error(errorMessage || t('appLayout.workspaceOpenFailed'), {
          duration: 5000
        });
      });
    }
  }, [openWorkspace, isAgenticMode, t]);

  // Initialize FlowChatManager: load history or create a default session
  React.useEffect(() => {
    const initializeFlowChat = async () => {
      if (!currentWorkspace?.rootPath) {
        return;
      }

      try {
        const preferredMode =
          sessionStorage.getItem('bitfun:flowchat:preferredMode') ||
          sessionStorage.getItem('bitfun:flowchat:lastMode') ||
          undefined;
        if (sessionStorage.getItem('bitfun:flowchat:preferredMode')) {
          sessionStorage.removeItem('bitfun:flowchat:preferredMode');
        }

        const flowChatManager = FlowChatManager.getInstance();
        const hasHistoricalSessions = await flowChatManager.initialize(
          currentWorkspace.rootPath,
          preferredMode
        );
        
        let sessionId: string | undefined;
        
        // If no history exists (or no active session was selected), create a default session.
        const { flowChatStore } = await import('@/flow_chat/store/FlowChatStore');
        if (!hasHistoricalSessions || !flowChatStore.getState().activeSessionId) {
          sessionId = await flowChatManager.createChatSession({}, preferredMode);
        }
        
        // Send pending project description from startup screen if present.
        const pendingDescription = sessionStorage.getItem('pendingProjectDescription');
        if (pendingDescription && pendingDescription.trim()) {
          sessionStorage.removeItem('pendingProjectDescription');
          
          // Wait briefly to ensure UI is fully rendered
          setTimeout(async () => {
            try {
              const targetSessionId = sessionId || flowChatStore.getState().activeSessionId;
              
              if (!targetSessionId) {
                log.error('Cannot find active session ID');
                return;
              }
              
              const fullMessage = t('appLayout.projectRequestMessage', { description: pendingDescription });
              await flowChatManager.sendMessage(fullMessage, targetSessionId);
              
              import('@/shared/notification-system').then(({ notificationService }) => {
                notificationService.success(t('appLayout.projectRequestSent'), {
                  duration: 3000
                });
              });
            } catch (sendError) {
              log.error('Failed to send project description', sendError);
              import('@/shared/notification-system').then(({ notificationService }) => {
                notificationService.error(t('appLayout.projectRequestSendFailed'), {
                  duration: 5000
                });
              });
            }
          }, 500);
        }
        // Open settings panel if requested during onboarding
        const pendingSettings = sessionStorage.getItem('pendingOpenSettings');
        if (pendingSettings) {
          sessionStorage.removeItem('pendingOpenSettings');
          
          setTimeout(async () => {
            try {
              const { quickActions } = await import('@/shared/services/ide-control');
              await quickActions.openSettings(pendingSettings);
            } catch (settingsError) {
              log.error('Failed to open pending settings', settingsError);
            }
          }, 500);
        }
      } catch (error) {
        log.error('FlowChatManager initialization failed', error);
        import('@/shared/notification-system').then(({ notificationService }) => {
          notificationService.error(t('appLayout.flowChatInitFailed'), {
            duration: 5000
          });
        });
      }
    };

    initializeFlowChat();
  }, [currentWorkspace?.rootPath, t]);

  // Save in-progress conversations on window close
  React.useEffect(() => {
    let unlistenFn: (() => void) | null = null;

    const setupWindowCloseListener = async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const currentWindow = getCurrentWindow();
        
        // Handle close request
        unlistenFn = await currentWindow.onCloseRequested(async (event: { preventDefault: () => void }) => {
          try {
            // Prevent immediate close
            event.preventDefault();
            
            // Save all in-progress turns
            const flowChatManager = FlowChatManager.getInstance();
            await flowChatManager.saveAllInProgressTurns();
            
            // Close after save completes
            await currentWindow.close();
          } catch (error) {
            log.error('Failed to save conversations, closing anyway', error);
            // Allow close even if save fails
            await currentWindow.close();
          }
        });
      } catch (error) {
        log.error('Failed to setup window close listener', error);
      }
    };

    setupWindowCloseListener();

    // Cleanup
    return () => {
      if (unlistenFn) {
        unlistenFn();
      }
    };
  }, []);

  // Note: expand-right-panel is handled by usePanelTabCoordinator.
  // Avoid duplicate listeners to prevent flicker.

  // Handle switch-to-files-panel event
  React.useEffect(() => {
    const handleSwitchToFilesPanel = () => {
      // Switch to files panel
      switchLeftPanelTab('files');
      
      // Expand left panel if collapsed
      if (state.layout.leftPanelCollapsed) {
        toggleLeftPanel();
      }
      
      // Expand right panel if collapsed (matches bottom bar files button)
      if (state.layout.rightPanelCollapsed) {
        setTimeout(() => {
          toggleRightPanel();
        }, 100);
      }
    };

    window.addEventListener('switch-to-files-panel', handleSwitchToFilesPanel);

    return () => {
      window.removeEventListener('switch-to-files-panel', handleSwitchToFilesPanel);
    };
  }, [state.layout.leftPanelCollapsed, state.layout.rightPanelCollapsed, switchLeftPanelTab, toggleLeftPanel, toggleRightPanel]);

  // Listen for Toolbar send message events
  React.useEffect(() => {
    const handleToolbarSendMessage = async (event: Event) => {
      const customEvent = event as CustomEvent<{ message: string; sessionId: string }>;
      const { message, sessionId } = customEvent.detail;
      
      if (message && sessionId) {
        try {
          const flowChatManager = FlowChatManager.getInstance();
          await flowChatManager.sendMessage(message, sessionId);
        } catch (error) {
          log.error('Failed to send toolbar message', error);
        }
      }
    };

    window.addEventListener('toolbar-send-message', handleToolbarSendMessage);

    return () => {
      window.removeEventListener('toolbar-send-message', handleToolbarSendMessage);
    };
  }, []);

  // Listen for Toolbar cancel task events
  React.useEffect(() => {
    const handleToolbarCancelTask = async () => {
      try {
        const flowChatManager = FlowChatManager.getInstance();
        await flowChatManager.cancelCurrentTask();
      } catch (error) {
        log.error('Failed to cancel toolbar task', error);
      }
    };

    window.addEventListener('toolbar-cancel-task', handleToolbarCancelTask);

    return () => {
      window.removeEventListener('toolbar-cancel-task', handleToolbarCancelTask);
    };
  }, []);

  // Create a FlowChat session (do not auto-open right panel)
  const handleCreateFlowChatSession = React.useCallback(async () => {
    try {
      const flowChatManager = FlowChatManager.getInstance();
      await flowChatManager.createChatSession({});
    } catch (error) {
      log.error('Failed to create FlowChat session', error);
    }
  }, []);

  // Listen for Toolbar create session events
  React.useEffect(() => {
    const handleToolbarCreateSession = () => {
      handleCreateFlowChatSession();
    };

    window.addEventListener('toolbar-create-session', handleToolbarCreateSession);

    return () => {
      window.removeEventListener('toolbar-create-session', handleToolbarCreateSession);
    };
  }, [handleCreateFlowChatSession]);

  // Enable global drag-and-drop
  React.useEffect(() => {
    // Initialize drag data at capture phase
    const handleDragStart = (e: DragEvent) => {
      // Set standard data and effectAllowed so the browser treats it as valid.
      if (e.dataTransfer) {
        if (e.dataTransfer.types.length === 0) {
          e.dataTransfer.setData('text/plain', 'dragging');
        }
        e.dataTransfer.effectAllowed = 'copy';
      }
    };
    
    const handleDragOver = (e: DragEvent) => {
      // Allow drop globally so the cursor indicates a valid drop target
      e.preventDefault();
    };
    
    const handleDragEnter = (e: DragEvent) => {
      // No-op
    };
    
    const handleDrop = (e: DragEvent) => {
      // Prevent default file open behavior globally
      if (!e.defaultPrevented) {
        e.preventDefault();
      }
    };
    
    // Register drag events (capture phase)
    document.addEventListener('dragstart', handleDragStart, true);
    document.addEventListener('dragover', handleDragOver, true);
    document.addEventListener('dragenter', handleDragEnter, true);
    document.addEventListener('drop', handleDrop, true);
    
    return () => {
      document.removeEventListener('dragstart', handleDragStart, true);
      document.removeEventListener('dragover', handleDragOver, true);
      document.removeEventListener('dragenter', handleDragEnter, true);
      document.removeEventListener('drop', handleDrop, true);
    };
  }, []);
  
  // Compose class names
  const containerClassName = [
    'bitfun-app-layout',
    className,
    !hasWorkspace ? 'bitfun-app-layout--startup-mode' : '',
    isTransitioning ? 'bitfun-app-layout--transitioning' : ''
  ].filter(Boolean).join(' ');
  
  // Toolbar mode: render ToolbarMode only
  if (isToolbarMode) {
    return <ToolbarMode />;
  }
  
  // Unified layout: content depends on workspace presence
  return (
    <div className={containerClassName} data-testid="app-layout">
      {/* Global header (always present) */}
      <Header
        className={`bitfun-app-header ${!hasWorkspace ? 'bitfun-app-header--startup-mode' : ''} ${isTransitioning ? 'bitfun-app-header--transitioning' : ''} ${isSweepGlowing ? 'bitfun-header--sweep-glow' : ''}`}
        onMinimize={handleMinimize}
        onMaximize={handleMaximize}
        onClose={handleClose}
        onHome={handleHomeClick}
        onToggleLeftPanel={toggleLeftPanel}
        onToggleRightPanel={toggleRightPanel}
        leftPanelCollapsed={state.layout.leftPanelCollapsed}
        rightPanelCollapsed={state.layout.rightPanelCollapsed}
        onCreateSession={handleCreateFlowChatSession}
        isMaximized={isMaximized}
      />

      {/* Main content */}
      <main className="bitfun-app-main-workspace" data-testid="app-main-content">
        {/* Render based on workspace presence */}
        {hasWorkspace ? (
          // With workspace: show workspace layout
          <WorkspaceLayout isEntering={isTransitioning || className.includes('page-entering') || className.includes('panels-sliding')} />
        ) : (
          // Without workspace: show startup content
          <StartupContent 
            onWorkspaceSelected={handleWorkspaceSelected}
            isTransitioning={isTransitioning}
          />
        )}
      </main>

      {/* Bottom bar (workspace only) */}
      {hasWorkspace && (
        <AppBottomBar 
          className="bitfun-app-bottom-bar"
        />
      )}
      
      {/* Standalone chat input (workspace + agentic mode only) */}
      {hasWorkspace && isAgenticMode && (
        <ChatInput 
          onSendMessage={(message: string) => {
            // Message dispatch is handled inside ChatInput
          }}
        />
      )}
    </div>
  );
};

export default AppLayout;
