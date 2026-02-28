/**
 * NavBar — navigation history controls + window chrome.
 *
 * Sits at the top of the left column, same height as SceneBar (32px).
 * Layout: [←][→]  <drag-region>  [_][□][×]
 *
 * - Back/Forward buttons mirror IDE navigation history.
 * - The centre strip is a drag region for moving the window.
 * - WindowControls (minimize/maximize/close) replace the old TitleBar chrome.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ArrowLeft, ArrowRight, FolderOpen, FolderPlus, History, Check } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useNavSceneStore } from '../../stores/navSceneStore';
import { useWorkspaceContext } from '../../../infrastructure/contexts/WorkspaceContext';
import { useI18n } from '../../../infrastructure/i18n';
import { PanelLeftIcon } from '../TitleBar/PanelIcons';
import { createLogger } from '@/shared/utils/logger';
import './NavBar.scss';

const log = createLogger('NavBar');

const INTERACTIVE_SELECTOR =
  'button, input, textarea, select, a, [role="button"], [contenteditable="true"], .window-controls, [role="menu"]';

interface NavBarProps {
  className?: string;
  isCollapsed?: boolean;
  onExpandNav?: () => void;
  onMaximize?: () => void;
}

const NavBar: React.FC<NavBarProps> = ({
  className = '',
  isCollapsed = false,
  onExpandNav,
  onMaximize,
}) => {
  const { t } = useI18n('common');
  const isMacOS = useMemo(() => {
    const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
    return (
      isTauri &&
      typeof navigator !== 'undefined' &&
      typeof navigator.platform === 'string' &&
      navigator.platform.toUpperCase().includes('MAC')
    );
  }, []);
  const showSceneNav = useNavSceneStore(s => s.showSceneNav);
  const navSceneId   = useNavSceneStore(s => s.navSceneId);
  const goBack       = useNavSceneStore(s => s.goBack);
  const goForward    = useNavSceneStore(s => s.goForward);
  const canGoBack    = showSceneNav && !!navSceneId;
  const canGoForward = !showSceneNav && !!navSceneId;
  const { currentWorkspace, recentWorkspaces, openWorkspace, switchWorkspace } = useWorkspaceContext();
  const [showLogoMenu, setShowLogoMenu] = useState(false);
  const [logoMenuClosing, setLogoMenuClosing] = useState(false);
  const [menuPos, setMenuPos] = useState<{ top: number; left: number }>({ top: 0, left: 0 });
  const containerRef = useRef<HTMLDivElement>(null);
  const menuPortalRef = useRef<HTMLDivElement>(null);
  const lastMouseDownTimeRef = useRef<number>(0);

  const handleBarMouseDown = useCallback((e: React.MouseEvent) => {
    const now = Date.now();
    const timeSinceLastMouseDown = now - lastMouseDownTimeRef.current;
    lastMouseDownTimeRef.current = now;

    if (e.button !== 0) return;
    const target = e.target as HTMLElement | null;
    if (!target) return;
    if (target.closest(INTERACTIVE_SELECTOR)) return;
    if (timeSinceLastMouseDown < 500 && timeSinceLastMouseDown > 50) return;

    void (async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        await getCurrentWindow().startDragging();
      } catch (error) {
        log.debug('startDragging failed', error);
      }
    })();
  }, []);

  const handleBarDoubleClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement | null;
    if (!target) return;
    if (target.closest(INTERACTIVE_SELECTOR)) return;
    onMaximize?.();
  }, [onMaximize]);

  const closeLogoMenu = useCallback(() => {
    setLogoMenuClosing(true);
    setTimeout(() => {
      setShowLogoMenu(false);
      setLogoMenuClosing(false);
    }, 150);
  }, []);

  const openLogoMenu = useCallback(() => {
    const btn = containerRef.current?.querySelector<HTMLElement>('.bitfun-nav-bar__logo-button');
    if (btn) {
      const rect = btn.getBoundingClientRect();
      setMenuPos({ top: rect.bottom + 4, left: rect.left });
    }
    setShowLogoMenu(true);
  }, []);

  useEffect(() => {
    if (!showLogoMenu) return;
    const onMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) return;
      if (containerRef.current?.contains(target)) return;
      if (menuPortalRef.current?.contains(target)) return;
      closeLogoMenu();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') closeLogoMenu();
    };
    document.addEventListener('mousedown', onMouseDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('mousedown', onMouseDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [showLogoMenu, closeLogoMenu]);

  const handleOpenProject = useCallback(async () => {
    closeLogoMenu();
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true, multiple: false }) as string;
      if (selected) await openWorkspace(selected);
    } catch {}
  }, [closeLogoMenu, openWorkspace]);

  const handleNewProject = useCallback(() => {
    closeLogoMenu();
    window.dispatchEvent(new CustomEvent('nav:new-project'));
  }, [closeLogoMenu]);

  const handleSwitchWorkspace = useCallback(async (workspaceId: string) => {
    const targetWorkspace = recentWorkspaces.find(item => item.id === workspaceId);
    if (!targetWorkspace) return;
    closeLogoMenu();
    try {
      await switchWorkspace(targetWorkspace);
    } catch {}
  }, [closeLogoMenu, recentWorkspaces, switchWorkspace]);
  const recentWorkspaceItems = useMemo(
    () =>
      recentWorkspaces.map((workspace) => (
        <Tooltip key={workspace.id} content={workspace.rootPath} placement="right" followCursor>
          <button
            type="button"
            className="bitfun-nav-bar__menu-item bitfun-nav-bar__menu-item--workspace"
            role="menuitem"
            onClick={() => {
              void handleSwitchWorkspace(workspace.id);
            }}
          >
            <FolderOpen size={13} aria-hidden="true" />
            <span className="bitfun-nav-bar__menu-item-main">{workspace.name}</span>
            {workspace.id === currentWorkspace?.id && <Check size={12} aria-hidden="true" />}
          </button>
        </Tooltip>
      )),
    [recentWorkspaces, handleSwitchWorkspace, currentWorkspace?.id]
  );

  const logoMenuPortal = showLogoMenu
    ? createPortal(
        <div
          ref={menuPortalRef}
          className={`bitfun-nav-bar__menu${logoMenuClosing ? ' is-closing' : ''}`}
          role="menu"
          style={{ top: menuPos.top, left: menuPos.left }}
        >
          {!isMacOS && (
            <>
              <button type="button" className="bitfun-nav-bar__menu-item" role="menuitem" onClick={() => { void handleOpenProject(); }}>
                <FolderOpen size={13} aria-hidden="true" />
                <span>{t('header.openProject')}</span>
              </button>
              <button type="button" className="bitfun-nav-bar__menu-item" role="menuitem" onClick={handleNewProject}>
                <FolderPlus size={13} aria-hidden="true" />
                <span>{t('header.newProject')}</span>
              </button>
              <div className="bitfun-nav-bar__menu-divider" role="separator" />
            </>
          )}
          <div className="bitfun-nav-bar__menu-section-title">
            <History size={12} aria-hidden="true" />
            <span>{t('header.recentWorkspaces')}</span>
          </div>

          {recentWorkspaceItems.length === 0 ? (
            <div className="bitfun-nav-bar__menu-empty">
              <span>{t('header.noRecentWorkspaces')}</span>
            </div>
          ) : (
            <div className="bitfun-nav-bar__menu-workspaces">{recentWorkspaceItems}</div>
          )}
        </div>,
        document.body
      )
    : null;

  const rootClassName = `bitfun-nav-bar${isCollapsed ? ' bitfun-nav-bar--collapsed' : ''}${isMacOS ? ' bitfun-nav-bar--macos' : ''} ${className}`;

  if (isCollapsed) {
    return (
      <div className={rootClassName} role="toolbar" aria-label={t('nav.aria.navControl')} onMouseDown={handleBarMouseDown} onDoubleClick={handleBarDoubleClick}>
        <div className="bitfun-nav-bar__logo-menu" ref={containerRef}>
          <button
            type="button"
            className="bitfun-nav-bar__logo-button"
            aria-label={t('header.openMenu')}
            aria-expanded={showLogoMenu}
            onClick={() => showLogoMenu ? closeLogoMenu() : openLogoMenu()}
            onContextMenu={(e) => {
              e.preventDefault();
              openLogoMenu();
            }}
          >
            <img
              src="/Logo-ICON.png"
              alt="BitFun"
              className="bitfun-nav-bar__logo"
              aria-hidden="true"
            />
          </button>
          {logoMenuPortal}
        </div>
        <Tooltip content={t('header.expandLeftPanel')} placement="bottom" followCursor>
          <button
            type="button"
            className="bitfun-nav-bar__panel-toggle"
            onClick={onExpandNav}
            aria-label={t('header.expandLeftPanel')}
          >
            <PanelLeftIcon size={13} />
          </button>
        </Tooltip>
      </div>
    );
  }

  return (
    <div className={rootClassName} role="toolbar" aria-label={t('nav.aria.navControl')} onMouseDown={handleBarMouseDown} onDoubleClick={handleBarDoubleClick}>
      <div className="bitfun-nav-bar__logo-menu" ref={containerRef}>
        <button
          type="button"
          className="bitfun-nav-bar__logo-button"
          aria-label={t('header.openMenu')}
          aria-expanded={showLogoMenu}
          onClick={() => showLogoMenu ? closeLogoMenu() : openLogoMenu()}
          onContextMenu={(e) => {
            e.preventDefault();
            openLogoMenu();
          }}
        >
          <img
            src="/Logo-ICON.png"
            alt="BitFun"
            className="bitfun-nav-bar__logo"
            aria-hidden="true"
          />
        </button>
        {logoMenuPortal}
      </div>

      {/* Back / Forward */}
      <Tooltip content={t('nav.backShortcut')} placement="bottom" followCursor disabled={!canGoBack}>
        <button
          className={`bitfun-nav-bar__btn${!canGoBack ? ' is-inactive' : ''}`}
          onClick={canGoBack ? goBack : undefined}
          aria-disabled={!canGoBack}
          aria-label={t('nav.back')}
        >
          <ArrowLeft size={15} />
        </button>
      </Tooltip>

      <Tooltip content={t('nav.forwardShortcut')} placement="bottom" followCursor disabled={!canGoForward}>
        <button
          className={`bitfun-nav-bar__btn${!canGoForward ? ' is-inactive' : ''}`}
          onClick={canGoForward ? goForward : undefined}
          aria-disabled={!canGoForward}
          aria-label={t('nav.forward')}
        >
          <ArrowRight size={15} />
        </button>
      </Tooltip>

    </div>
  );
};

export default NavBar;
