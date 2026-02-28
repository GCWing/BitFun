/**
 * TitleBar — application title bar.
 *
 * Layout: [Logo/Menu] — [Center: title or search] — [Notification | Settings | WindowControls]
 *
 * Panel toggle group removed (moved to SceneBar / scene level).
 * NotificationButton added from StatusBar.
 */

import React, { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import { Settings, FolderOpen, Home, FolderPlus, Info } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import { useWorkspaceContext } from '../../../infrastructure/contexts/WorkspaceContext';
import './TitleBar.scss';

import { Button, WindowControls, Tooltip } from '@/component-library';
import { WorkspaceManager } from '../../../tools/workspace';
import { CurrentSessionTitle, useToolbarModeContext } from '../../../flow_chat';
import { createConfigCenterTab } from '@/shared/utils/tabUtils';
import { workspaceAPI } from '@/infrastructure/api';
import { NewProjectDialog } from '../NewProjectDialog';
import { AboutDialog } from '../AboutDialog';
import { GlobalSearch } from './GlobalSearch';
import { AgentOrb } from './AgentOrb';
import NotificationButton from './NotificationButton';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('TitleBar');

interface TitleBarProps {
  className?: string;
  onMinimize: () => void;
  onMaximize: () => void;
  onClose: () => void;
  onHome: () => void;
  onCreateSession?: () => void;
  isMaximized?: boolean;
}

const TitleBar: React.FC<TitleBarProps> = ({
  className = '',
  onMinimize,
  onMaximize,
  onClose,
  onHome,
  onCreateSession,
  isMaximized = false,
}) => {
  const { t } = useTranslation('common');
  const [showWorkspaceStatus, setShowWorkspaceStatus] = useState(false);
  const [showNewProjectDialog, setShowNewProjectDialog] = useState(false);
  const [showAboutDialog, setShowAboutDialog] = useState(false);
  const [showLogoMenu, setShowLogoMenu] = useState(false);
  const [isOrbHovered, setIsOrbHovered] = useState(false);
  const logoMenuContainerRef = useRef<HTMLDivElement | null>(null);

  const isMacOS = useMemo(() => {
    const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
    return (
      isTauri &&
      typeof navigator !== 'undefined' &&
      typeof navigator.platform === 'string' &&
      navigator.platform.toUpperCase().includes('MAC')
    );
  }, []);

  const { enableToolbarMode } = useToolbarModeContext();

  const lastMouseDownTimeRef = React.useRef<number>(0);

  const handleHeaderMouseDown = useCallback((e: React.MouseEvent) => {
    const now = Date.now();
    const timeSinceLastMouseDown = now - lastMouseDownTimeRef.current;
    lastMouseDownTimeRef.current = now;

    if (e.button !== 0) return;

    const target = e.target as HTMLElement | null;
    if (!target) return;

    if (
      target.closest(
        'button, input, textarea, select, a, [role="button"], [contenteditable="true"], .window-controls, .agent-orb-wrapper, .agent-orb-logo'
      )
    ) {
      return;
    }

    if (timeSinceLastMouseDown < 500 && timeSinceLastMouseDown > 50) {
      return;
    }

    void (async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        await getCurrentWindow().startDragging();
      } catch (error) {
        log.debug('startDragging failed', error);
      }
    })();
  }, []);

  const handleHeaderDoubleClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement | null;
    if (!target) return;

    if (
      target.closest(
        'button, input, textarea, select, a, [role="button"], [contenteditable="true"], .window-controls, .agent-orb-wrapper, .agent-orb-logo'
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

  const handleNewProject = useCallback(() => {
    setShowNewProjectDialog(true);
  }, []);

  const handleConfirmNewProject = useCallback(async (parentPath: string, projectName: string) => {
    const normalizedParentPath = parentPath.replace(/\\/g, '/');
    const newProjectPath = `${normalizedParentPath}/${projectName}`;

    log.info('Creating new project', { parentPath, projectName, fullPath: newProjectPath });

    try {
      await workspaceAPI.createDirectory(newProjectPath);
      await openWorkspace(newProjectPath);
      log.info('New project opened', { path: newProjectPath });
    } catch (error) {
      log.error('Failed to create project', error);
      throw error;
    }
  }, [openWorkspace]);

  const handleGoHome = useCallback(() => {
    onHome();
  }, [onHome]);

  const handleShowAbout = useCallback(() => {
    setShowAboutDialog(true);
  }, []);

  const handleMenuClick = useCallback(() => {
    setShowLogoMenu((prev) => !prev);
  }, []);

  const handleOrbHoverEnter = useCallback(() => {
    setIsOrbHovered(true);
  }, []);

  const handleOrbHoverLeave = useCallback(() => {
    setIsOrbHovered(false);
  }, []);

  // Listen for nav panel events dispatched by WorkspaceHeader
  useEffect(() => {
    const onNewProject = () => handleNewProject();
    const onGoHome = () => handleGoHome();
    window.addEventListener('nav:new-project', onNewProject);
    window.addEventListener('nav:go-home', onGoHome);
    return () => {
      window.removeEventListener('nav:new-project', onNewProject);
      window.removeEventListener('nav:go-home', onGoHome);
    };
  }, [handleNewProject, handleGoHome]);

  const menuOrbNode = (
    <div
      className="agent-orb-wrapper"
      onMouseEnter={handleOrbHoverEnter}
      onMouseLeave={handleOrbHoverLeave}
    >
      <AgentOrb
        isAgenticMode={false}
        onToggle={handleMenuClick}
        tooltipText={showLogoMenu ? t('header.closeMenu') : t('header.openMenu')}
      />
    </div>
  );

  // macOS menubar events
  useEffect(() => {
    if (!isMacOS) return;

    let unlistenFns: Array<() => void> = [];

    void (async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');

        unlistenFns.push(await listen('bitfun_menu_open_project', () => { void handleOpenProject(); }));
        unlistenFns.push(await listen('bitfun_menu_new_project', () => { handleNewProject(); }));
        unlistenFns.push(await listen('bitfun_menu_go_home', () => { handleGoHome(); }));
        unlistenFns.push(await listen('bitfun_menu_about', () => { handleShowAbout(); }));
      } catch (error) {
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
      if (event.key === 'Escape') setShowLogoMenu(false);
    };

    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEscape);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [showLogoMenu]);

  const horizontalMenuItems = [
    { id: 'open-project', label: t('header.openProject'), icon: <FolderOpen size={14} />, onClick: handleOpenProject },
    { id: 'new-project',  label: t('header.newProject'),  icon: <FolderPlus size={14} />, onClick: handleNewProject  },
    { id: 'go-home',      label: t('header.goHome'),      icon: <Home size={14} />,       onClick: handleGoHome, testId: 'header-home-btn' },
    { id: 'about',        label: t('header.about'),       icon: <Info size={14} />,       onClick: handleShowAbout   },
  ];

  const orbGlowClass = isOrbHovered ? 'bitfun-header--orb-glow-editor' : '';

  return (
    <>
      <header
        className={`${className} ${isMacOS ? 'bitfun-app-header--macos-native-titlebar' : ''} ${orbGlowClass}`}
        data-testid="header-container"
        onMouseDown={handleHeaderMouseDown}
        onDoubleClick={handleHeaderDoubleClick}
      >
        {/* Left: Logo / menu */}
        <div className="bitfun-header-left">
          {!isMacOS && (
            <div className="bitfun-menu-container" ref={logoMenuContainerRef}>
              {menuOrbNode}

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

        {/* Center: session title or search */}
        <div className="bitfun-header-center">
          <CurrentSessionTitle onCreateSession={onCreateSession} />
        </div>

        {/* Right: Notification + Settings + WindowControls */}
        <div className="bitfun-header-right">
          <NotificationButton />

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

      <NewProjectDialog
        isOpen={showNewProjectDialog}
        onClose={() => setShowNewProjectDialog(false)}
        onConfirm={handleConfirmNewProject}
        defaultParentPath={hasWorkspace ? workspacePath : undefined}
      />

      <AboutDialog
        isOpen={showAboutDialog}
        onClose={() => setShowAboutDialog(false)}
      />

      <WorkspaceManager
        isVisible={showWorkspaceStatus}
        onClose={() => setShowWorkspaceStatus(false)}
        onWorkspaceSelect={(workspace: any) => {
          log.debug('Workspace selected', { workspace });
        }}
      />
    </>
  );
};

export default TitleBar;
