import React, { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import { Settings, FolderOpen, Home, FolderPlus, Info, PanelBottom } from 'lucide-react';
import { PanelLeftIcon, PanelRightIcon, PanelCenterIcon } from './PanelIcons';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import { useWorkspaceContext } from '../../../infrastructure/contexts/WorkspaceContext';
import { useViewMode } from '../../../infrastructure/contexts/ViewModeContext';
import './Header.scss';

import { Button, WindowControls, Tooltip } from '@/component-library';
import { WorkspaceManager } from '../../../tools/workspace';
import { CurrentSessionTitle, useToolbarModeContext } from '../../../flow_chat'; // Imported from flow_chat module
import { createConfigCenterTab } from '@/shared/utils/tabUtils';
import { workspaceAPI } from '@/infrastructure/api';
import { NewProjectDialog } from '../NewProjectDialog';
import { AboutDialog } from '../AboutDialog';
import { GlobalSearch } from './GlobalSearch';
import { AgentOrb } from './AgentOrb';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('Header');

interface HeaderProps {
  className?: string;
  onMinimize: () => void;
  onMaximize: () => void;
  onClose: () => void;
  onHome: () => void;
  onToggleLeftPanel: () => void;
  onToggleChatPanel: () => void;
  onToggleRightPanel: () => void;
  leftPanelCollapsed: boolean;
  chatCollapsed: boolean;
  rightPanelCollapsed: boolean;
  onCreateSession?: () => void; // Callback to create a FlowChat session
  isMaximized?: boolean; // Whether the window is maximized
}

/**
 * Application header component.
 * Includes title bar, toolbar, and window controls.
 */
const Header: React.FC<HeaderProps> = ({
  className = '',
  onMinimize,
  onMaximize,
  onClose,
  onHome,
  onToggleLeftPanel,
  onToggleChatPanel,
  onToggleRightPanel,
  leftPanelCollapsed,
  chatCollapsed,
  rightPanelCollapsed,
  onCreateSession,
  isMaximized = false
}) => {
  const { t } = useTranslation('common');
  const [showWorkspaceStatus, setShowWorkspaceStatus] = useState(false);
  const [showNewProjectDialog, setShowNewProjectDialog] = useState(false);
  const [showAboutDialog, setShowAboutDialog] = useState(false);
  const [showLogoMenu, setShowLogoMenu] = useState(false);
  const [isOrbHovered, setIsOrbHovered] = useState(false); // Orb hover state
  const logoMenuContainerRef = useRef<HTMLDivElement | null>(null);

  // macOS Desktop (Tauri): use native titlebar traffic lights (hide custom window controls)
  const isMacOS = useMemo(() => {
    const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
    return (
      isTauri &&
      typeof navigator !== 'undefined' &&
      typeof navigator.platform === 'string' &&
      navigator.platform.toUpperCase().includes('MAC')
    );
  }, []);
  
  // View mode
  const { setViewMode, isCoworkMode, isCoderMode } = useViewMode();
  
	// Toolbar mode
	const { enableToolbarMode } = useToolbarModeContext();

	// Track last mousedown time to detect double-clicks
	const lastMouseDownTimeRef = React.useRef<number>(0);

	  // Cross-platform frameless window: use startDragging() for titlebar drag (avoid data-tauri-drag-region)
	  const handleHeaderMouseDown = useCallback((e: React.MouseEvent) => {
			const now = Date.now();
			const timeSinceLastMouseDown = now - lastMouseDownTimeRef.current;
			lastMouseDownTimeRef.current = now;

		// Left-click only
		if (e.button !== 0) return;

		const target = e.target as HTMLElement | null;
		if (!target) return;

		// Do not start drag on interactive elements
		if (
			target.closest(
				'button, input, textarea, select, a, [role="button"], [contenteditable="true"], .window-controls, .bitfun-immersive-panel-toggles, .agent-orb-wrapper, .agent-orb-logo'
			)
		) {
			return;
		}

		// If this is a potential double-click (<500ms), skip drag to allow the dblclick event
		if (timeSinceLastMouseDown < 500 && timeSinceLastMouseDown > 50) {
			return;
		}

			void (async () => {
				try {
					const { getCurrentWindow } = await import('@tauri-apps/api/window');
					await getCurrentWindow().startDragging();
			} catch (error) {
				// May fail outside Tauri (e.g., web preview); ignore silently
				log.debug('startDragging failed', error);
			}
			})();
		}, []);

		// Double-click empty titlebar area: match WindowControls maximize behavior
		const handleHeaderDoubleClick = useCallback((e: React.MouseEvent) => {
		const target = e.target as HTMLElement | null;
		if (!target) return;

		if (
			target.closest(
				'button, input, textarea, select, a, [role="button"], [contenteditable="true"], .window-controls, .bitfun-immersive-panel-toggles, .agent-orb-wrapper, .agent-orb-logo'
			)
		) {
			return;
		}

			onMaximize();
		}, [onMaximize]);
  
  const {
    hasWorkspace,
    workspacePath,
    openWorkspace
  } = useWorkspaceContext();

  // Open existing project
  const handleOpenProject = useCallback(async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('header.selectProjectDirectory')
      }) as string;

      if (selected && typeof selected === 'string') {
        await openWorkspace(selected);
        log.info('Opening workspace', { path: selected });
      }
    } catch (error) {
      log.error('Failed to open workspace', error);
    }
  }, [openWorkspace]);

  // Open the new project dialog
  const handleNewProject = useCallback(() => {
    setShowNewProjectDialog(true);
  }, []);

  // Confirm creation of a new project
  const handleConfirmNewProject = useCallback(async (parentPath: string, projectName: string) => {
    const normalizedParentPath = parentPath.replace(/\\/g, '/');
    const newProjectPath = `${normalizedParentPath}/${projectName}`;
    
    log.info('Creating new project', { parentPath, projectName, fullPath: newProjectPath });
    
    try {
      // Create directory
      await workspaceAPI.createDirectory(newProjectPath);
      
      // Open the newly created project
      await openWorkspace(newProjectPath);
      log.info('New project opened', { path: newProjectPath });
      
    } catch (error) {
      log.error('Failed to create project', error);
      throw error; // Re-throw so the dialog can display the error
    }
  }, [openWorkspace]);

  // Return to home
  const handleGoHome = useCallback(() => {
    onHome();
  }, [onHome]);

  // Open the About dialog
  const handleShowAbout = useCallback(() => {
    setShowAboutDialog(true);
  }, []);

  // Orb click: toggle popup menu open/close
  const handleMenuClick = useCallback(() => {
    setShowLogoMenu((prev) => !prev);
  }, []);

  // Orb hover: enable header glow
  const handleOrbHoverEnter = useCallback(() => {
    setIsOrbHovered(true);
  }, []);

  const handleOrbHoverLeave = useCallback(() => {
    setIsOrbHovered(false);
  }, []);

  const menuOrbNode = (
    <div
      className="agent-orb-wrapper"
      onMouseEnter={handleOrbHoverEnter}
      onMouseLeave={handleOrbHoverLeave}
    >
      <AgentOrb
        isAgenticMode={isCoworkMode}
        onToggle={handleMenuClick}
        tooltipText={showLogoMenu ? t('header.closeMenu') : t('header.openMenu')}
      />
    </div>
  );

  const modeSwitchNode = (
    <div
      className={`bitfun-mode-switch ${isCoderMode ? 'bitfun-mode-switch--coder' : 'bitfun-mode-switch--cowork'}`}
      role="group"
      aria-label={t('header.modeSwitchAriaLabel')}
    >
      <Tooltip content={t('header.switchToCoder')} placement="bottom">
        <button
          type="button"
          className={`bitfun-mode-switch__btn ${isCoderMode ? 'bitfun-mode-switch__btn--active' : ''}`}
          onClick={() => setViewMode('coder')}
          aria-pressed={isCoderMode}
        >
          {t('header.modeCoder')}
        </button>
      </Tooltip>
      <span className="bitfun-mode-switch__divider" aria-hidden="true">
        /
      </span>
      <Tooltip content={t('header.switchToCowork')} placement="bottom">
        <button
          type="button"
          className={`bitfun-mode-switch__btn ${isCoworkMode ? 'bitfun-mode-switch__btn--active' : ''}`}
          onClick={() => setViewMode('cowork')}
          aria-pressed={isCoworkMode}
        >
          {t('header.modeCowork')}
        </button>
      </Tooltip>
    </div>
  );

  // macOS menubar events (Tauri native menubar)
  useEffect(() => {
    if (!isMacOS) return;

    let unlistenFns: Array<() => void> = [];

    void (async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');

        unlistenFns.push(
          await listen('bitfun_menu_open_project', () => {
            void handleOpenProject();
          })
        );
        unlistenFns.push(
          await listen('bitfun_menu_new_project', () => {
            handleNewProject();
          })
        );
        unlistenFns.push(
          await listen('bitfun_menu_go_home', () => {
            handleGoHome();
          })
        );
        unlistenFns.push(
          await listen('bitfun_menu_about', () => {
            handleShowAbout();
          })
        );
      } catch (error) {
        // May fail outside Tauri (e.g., web preview); ignore silently
        log.debug('menubar listen failed', error);
      }
    })();

    return () => {
      unlistenFns.forEach((fn) => fn());
      unlistenFns = [];
    };
  }, [isMacOS, handleOpenProject, handleNewProject, handleGoHome, handleShowAbout]);

  // Close popup menu on outside click / Escape
  useEffect(() => {
    if (!showLogoMenu) return;

    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) return;
      if (logoMenuContainerRef.current?.contains(target)) return;
      setShowLogoMenu(false);
    };

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setShowLogoMenu(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEscape);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [showLogoMenu]);

  // Horizontal menu items (no separators)
  const horizontalMenuItems = [
    {
      id: 'open-project',
      label: t('header.openProject'),
      icon: <FolderOpen size={14} />,
      onClick: handleOpenProject
    },
    {
      id: 'new-project',
      label: t('header.newProject'),
      icon: <FolderPlus size={14} />,
      onClick: handleNewProject
    },
    {
      id: 'go-home',
      label: t('header.goHome'),
      icon: <Home size={14} />,
      onClick: handleGoHome,
      testId: 'header-home-btn'
    },
    {
      id: 'about',
      label: t('header.about'),
      icon: <Info size={14} />,
      onClick: handleShowAbout
    }
  ];

		return (
			<>
						<header 
							className={`${className} ${isMacOS ? 'bitfun-app-header--macos-native-titlebar' : ''} ${isOrbHovered ? (isCoworkMode ? 'bitfun-header--orb-glow-agentic' : 'bitfun-header--orb-glow-editor') : ''}`} 
							data-testid="header-container"
							onMouseDown={handleHeaderMouseDown}
							onDoubleClick={handleHeaderDoubleClick}
						>
						<div className="bitfun-header-left">
              {/* macOS: move items to system menubar; hide custom menu button; move toggle to right */}
              {!isMacOS && (
                <div className="bitfun-menu-container" ref={logoMenuContainerRef}>
                  {/* Logo: used as the menu trigger */}
                  {menuOrbNode}
                  {modeSwitchNode}

                  {/* Popup menu items */}
                  <div
                    className={`bitfun-logo-popup-menu ${showLogoMenu ? 'bitfun-logo-popup-menu--visible' : ''}`}
                    role="menu"
                  >
                    {horizontalMenuItems.map((item, index) => (
                      <React.Fragment key={item.id}>
                        {index > 0 && <div className="bitfun-logo-popup-menu-divider" />}
                        <button
                          className="bitfun-logo-popup-menu-item"
                          role="menuitem"
                          onClick={() => {
                            item.onClick();
                            setShowLogoMenu(false);
                          }}
                          data-testid={(item as any).testId}
                        >
                          {item.icon}
                          <span className="bitfun-logo-popup-menu-item__label">{item.label}</span>
                        </button>
                      </React.Fragment>
                    ))}
                  </div>
                </div>
              )}
	        </div>
						
						<div 
							className="bitfun-header-center"
					>
						{/* Current session title: show whenever chat panel is visible */}
						{!chatCollapsed && <CurrentSessionTitle onCreateSession={onCreateSession} />}
						
          {/* Global search: show in coder layout when chat is hidden */}
          {chatCollapsed && <GlobalSearch />}
        </div>
        
        <div className="bitfun-header-right">
          {/* Panel flow controls: keep three independent toggles with richer flow semantics */}
          <div
            className={[
              'bitfun-immersive-panel-toggles',
              'bitfun-immersive-panel-toggles--flow',
              !leftPanelCollapsed && 'bitfun-immersive-panel-toggles--left-open',
              !chatCollapsed && 'bitfun-immersive-panel-toggles--chat-open',
              !rightPanelCollapsed && 'bitfun-immersive-panel-toggles--right-open',
              chatCollapsed && 'bitfun-immersive-panel-toggles--chat-collapsed'
            ].filter(Boolean).join(' ')}
          >
            <span className="bitfun-flow-rail bitfun-flow-rail--left" aria-hidden="true" />
            <span className="bitfun-flow-rail bitfun-flow-rail--right" aria-hidden="true" />

            <Tooltip content={leftPanelCollapsed ? t('header.expandLeftPanel') : t('header.collapseLeftPanel')} placement="bottom">
              <button
                type="button"
                className={`bitfun-panel-indicator bitfun-panel-indicator--left ${!leftPanelCollapsed ? 'active' : ''}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onToggleLeftPanel();
                }}
              >
                <PanelLeftIcon size={14} filled={!leftPanelCollapsed} />
              </button>
            </Tooltip>

            <Tooltip content={chatCollapsed ? t('header.showChatPanel') : t('header.hideChatPanel')} placement="bottom">
              <button
                type="button"
                className={`bitfun-panel-indicator bitfun-panel-indicator--chat ${!chatCollapsed ? 'active' : ''}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onToggleChatPanel();
                }}
              >
                <PanelCenterIcon size={14} filled={!chatCollapsed} />
              </button>
            </Tooltip>

            <Tooltip content={rightPanelCollapsed ? t('header.expandRightPanel') : t('header.collapseRightPanel')} placement="bottom">
              <button
                type="button"
                className={`bitfun-panel-indicator bitfun-panel-indicator--right ${!rightPanelCollapsed ? 'active' : ''}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onToggleRightPanel();
                }}
              >
                <PanelRightIcon size={14} filled={!rightPanelCollapsed} />
              </button>
            </Tooltip>

            {/* Toolbar mode toggle: available when chat panel is visible */}
            {!chatCollapsed && (
              <Tooltip content={t('header.switchToToolbar')}>
                <button
                  type="button"
                  className="bitfun-panel-indicator bitfun-panel-indicator--toolbar"
                  onClick={(e) => {
                    e.stopPropagation();
                    enableToolbarMode();
                  }}
                >
                  <PanelBottom size={14} />
                </button>
              </Tooltip>
            )}
          </div>
	          
	          {/* Config center button */}
	          <Tooltip content={t('header.configCenter')}>
            <Button
              variant="ghost"
              size="small"
              iconOnly
              data-testid="header-config-btn"
              onClick={() => {
                createConfigCenterTab('models', 'agent');
              }}
            >
              <Settings size={14} />
            </Button>
	          </Tooltip>

            {/* macOS: keep mode switch accessible in the right section */}
            {isMacOS && modeSwitchNode}
		          
		          {/* Window controls (macOS uses native traffic lights; hide custom buttons) */}
		          {!isMacOS && (
		            <WindowControls
	              onMinimize={onMinimize}
	              onMaximize={onMaximize}
	              onClose={onClose}
	              isMaximized={isMaximized}
	              data-testid-minimize="header-minimize-btn"
	              data-testid-maximize="header-maximize-btn"
	              data-testid-close="header-close-btn"
	            />
	          )}
	        </div>
	      </header>



      {/* New project dialog */}
      <NewProjectDialog
        isOpen={showNewProjectDialog}
        onClose={() => setShowNewProjectDialog(false)}
        onConfirm={handleConfirmNewProject}
        defaultParentPath={hasWorkspace ? workspacePath : undefined}
      />

      {/* About dialog */}
      <AboutDialog
        isOpen={showAboutDialog}
        onClose={() => setShowAboutDialog(false)}
      />

      {/* Workspace status modal */}
      <WorkspaceManager 
        isVisible={showWorkspaceStatus}
        onClose={() => setShowWorkspaceStatus(false)}
        onWorkspaceSelect={(workspace: any) => {
          log.debug('Workspace selected', { workspace });
          // Workspace selection is handled in the useWorkspace hook
        }}
      />
    </>
  );
};

export default Header;
