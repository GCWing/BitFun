/**
 * UnifiedTopBar — full-width application top bar.
 *
 * Replaces the former split chrome (NavBar in the left column + SceneHeaderBar
 * in the scene area) with a single bar spanning the entire window width.
 *
 * Layout (left → right):
 *   [macOS traffic-lights reserve] [Logo▼ menu: toolbar, about]
 *   [Arrow back button (conditional)] [context title] ─drag─
 *   [search trigger] ─drag─ [📱 Remote] [_][□][×]
 *
 * Unified back button / title logic:
 *   - overlay active          → back closes overlay + overlay scene title
 *   - non-Dispatcher session  → back opens Agentic OS (Dispatcher) + session mode / workspace
 *   - Dispatcher session      → no back button, no title (logo-only chrome)
 *   - no session              → nothing extra shown
 *
 * The empty areas between interactive elements act as Tauri window-drag regions.
 */

import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ArrowLeft, Info, ListChecks, PictureInPicture2, Search, FolderOpen } from 'lucide-react';
import { Modal, Tooltip, WindowControls } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useToolbarModeContext } from '@/flow_chat/components/toolbar-mode/ToolbarModeContext';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useNotification } from '@/shared/notification-system';
import { RemoteConnectDialog } from '../RemoteConnectDialog';
import {
  RemoteConnectDisclaimerContent,
} from '../RemoteConnectDialog/RemoteConnectDisclaimer';
import {
  getRemoteConnectDisclaimerAgreed,
  setRemoteConnectDisclaimerAgreed,
} from '../RemoteConnectDialog/remoteConnectDisclaimerStorage';
import { useOverlayStore } from '../../stores/overlayStore';
import { useHeaderStore } from '../../stores/headerStore';
import { useSessionCapsuleStore } from '../../stores/sessionCapsuleStore';
import SessionListDialog from '../SessionListDialog/SessionListDialog';
import { getOverlayDef } from '../../overlay/overlayRegistry';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { ALL_SHORTCUTS } from '@/shared/constants/shortcuts';
import { createLogger } from '@/shared/utils/logger';
import { openDispatcherSession } from '@/flow_chat/services/openDispatcherSession';
import {
  remoteConnectAPI,
  type RemoteConnectStatus,
} from '@/infrastructure/api/service-api/RemoteConnectAPI';
import RemoteControlButton from './RemoteControlButton';
import GlobalSearchDialog from '../GlobalSearchDialog/GlobalSearchDialog';
import type { OverlaySceneId } from '../../overlay/types';
import './UnifiedTopBar.scss';

const log = createLogger('UnifiedTopBar');

const NAV_TOGGLE_SEARCH_DEF = ALL_SHORTCUTS.find((d) => d.id === 'nav.toggleSearch')!;

const INTERACTIVE_SELECTOR =
  'button, input, textarea, select, a, [role="button"], [contenteditable="true"], .window-controls, [role="menu"]';

export interface UnifiedTopBarProps {
  activeOverlay: OverlaySceneId | null;
  onMinimize?: () => void;
  onMaximize?: () => void;
  onClose?: () => void;
  isMaximized?: boolean;
}

const UnifiedTopBar: React.FC<UnifiedTopBarProps> = ({
  activeOverlay,
  onMinimize,
  onMaximize,
  onClose,
  isMaximized = false,
}) => {
  const { t } = useI18n('common');
  const { enableToolbarMode } = useToolbarModeContext();
  const { hasWorkspace } = useCurrentWorkspace();
  const { warning } = useNotification();
  const closeOverlay = useOverlayStore((s) => s.closeOverlay);
  const sessionContext = useHeaderStore((s) => s.sessionContext);
  const openSessionListDialog = useSessionCapsuleStore((s) => s.openSessionListDialog);
  const hasWindowControls = !!(onMinimize && onMaximize && onClose);
  const hasOverlay = activeOverlay !== null;

  const [searchOpen, setSearchOpen] = useState(false);
  const [logoMenuOpen, setLogoMenuOpen] = useState(false);
  const [logoMenuPosition, setLogoMenuPosition] = useState<{ top: number; left: number } | null>(null);
  const [showRemoteConnect, setShowRemoteConnect] = useState(false);
  const [showRemoteDisclaimer, setShowRemoteDisclaimer] = useState(false);
  const [hasAgreedRemoteDisclaimer, setHasAgreedRemoteDisclaimer] = useState<boolean>(() =>
    getRemoteConnectDisclaimerAgreed()
  );
  const [remoteConnectStatus, setRemoteConnectStatus] = useState<RemoteConnectStatus | null>(null);
  const logoMenuAnchorRef = useRef<HTMLDivElement>(null);
  const lastMouseDownTimeRef = useRef<number>(0);

  const closeLogoMenu = useCallback(() => {
    setLogoMenuOpen(false);
  }, []);

  const updateLogoMenuPosition = useCallback(() => {
    const anchor = logoMenuAnchorRef.current;
    if (!anchor) return;

    const rect = anchor.getBoundingClientRect();
    const viewportPadding = 8;
    const estimatedWidth = 240;
    const maxLeft = window.innerWidth - estimatedWidth - viewportPadding;

    setLogoMenuPosition({
      top: Math.max(viewportPadding, rect.bottom + 6),
      left: Math.max(viewportPadding, Math.min(rect.left, maxLeft)),
    });
  }, []);

  useLayoutEffect(() => {
    if (!logoMenuOpen) {
      setLogoMenuPosition(null);
      return;
    }
    updateLogoMenuPosition();
  }, [logoMenuOpen, updateLogoMenuPosition]);

  useEffect(() => {
    if (!logoMenuOpen) return;

    const handleViewportChange = () => updateLogoMenuPosition();
    window.addEventListener('resize', handleViewportChange);
    window.addEventListener('scroll', handleViewportChange, true);

    return () => {
      window.removeEventListener('resize', handleViewportChange);
      window.removeEventListener('scroll', handleViewportChange, true);
    };
  }, [logoMenuOpen, updateLogoMenuPosition]);

  const handleLogoAbout = useCallback(() => {
    closeLogoMenu();
    window.dispatchEvent(new CustomEvent('nav:show-about'));
  }, [closeLogoMenu]);

  const handleFloatingMode = useCallback(() => {
    closeLogoMenu();
    enableToolbarMode();
  }, [closeLogoMenu, enableToolbarMode]);

  const handleRemoteConnect = useCallback(async () => {
    if (!hasWorkspace) {
      warning(t('header.remoteConnectRequiresWorkspace'));
      return;
    }

    closeLogoMenu();

    if (hasAgreedRemoteDisclaimer || getRemoteConnectDisclaimerAgreed()) {
      setHasAgreedRemoteDisclaimer(true);
      setShowRemoteConnect(true);
      return;
    }

    setShowRemoteDisclaimer(true);
  }, [hasWorkspace, warning, t, closeLogoMenu, hasAgreedRemoteDisclaimer]);

  const handleAgreeDisclaimer = useCallback(() => {
    setRemoteConnectDisclaimerAgreed();
    setHasAgreedRemoteDisclaimer(true);
    setShowRemoteDisclaimer(false);
    setShowRemoteConnect(true);
  }, []);

  useEffect(() => {
    if (!logoMenuOpen) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        closeLogoMenu();
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [logoMenuOpen, closeLogoMenu]);

  const isTauriDesktop = useMemo(
    () => typeof window !== 'undefined' && '__TAURI__' in window,
    []
  );

  const isMacOS = useMemo(() => {
    return (
      isTauriDesktop &&
      typeof navigator !== 'undefined' &&
      typeof navigator.platform === 'string' &&
      navigator.platform.toUpperCase().includes('MAC')
    );
  }, [isTauriDesktop]);

  useEffect(() => {
    if (!isTauriDesktop) return;

    let cancelled = false;
    const poll = async () => {
      try {
        const s = await remoteConnectAPI.getStatus();
        if (!cancelled) setRemoteConnectStatus(s);
      } catch {
        if (!cancelled) setRemoteConnectStatus(null);
      }
    };

    void poll();
    const id = window.setInterval(poll, 4000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [isTauriDesktop]);

  const toggleNavSearch = useCallback(() => {
    setSearchOpen((v) => !v);
  }, []);

  useShortcut(NAV_TOGGLE_SEARCH_DEF.id, NAV_TOGGLE_SEARCH_DEF.config, toggleNavSearch, {
    priority: 5,
    description: NAV_TOGGLE_SEARCH_DEF.descriptionKey,
  });

  // Secondary binding: Alt+F (same action as Mod+K, not listed separately in keyboard settings)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey || e.key.toLowerCase() !== 'f') return;
      e.preventDefault();
      toggleNavSearch();
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [toggleNavSearch]);

  const overlayDef = hasOverlay ? getOverlayDef(activeOverlay) : null;
  const overlayTitle = overlayDef?.labelKey ? t(overlayDef.labelKey) : (overlayDef?.label ?? '');

  // ── Unified context nav (back button + title) ─────────────────────────────
  const isDispatcherSession =
    sessionContext?.mode === 'Dispatcher' ||
    sessionContext?.mode?.toLowerCase() === 'dispatcher';

  const sessionWorkspaceName = useMemo(() => {
    const explicit = sessionContext?.workspaceDisplayName?.trim();
    if (explicit) return explicit;
    const p = sessionContext?.workspacePath;
    if (!p) return '';
    return p.replace(/\\/g, '/').split('/').filter(Boolean).pop() ?? p;
  }, [sessionContext?.workspacePath, sessionContext?.workspaceDisplayName]);

  const sessionTitle = useMemo(() => {
    if (!sessionContext) return '';
    if (isDispatcherSession) return '';
    const label = sessionContext.mode ?? '';
    return sessionWorkspaceName ? `${label} / ${sessionWorkspaceName}` : label;
  }, [sessionContext, isDispatcherSession, sessionWorkspaceName]);

  // Back + title: overlays always; base layer only for non-Dispatcher sessions (hide "Agentic OS" on Dispatcher).
  const showContextNav = hasOverlay || (!!sessionContext && !isDispatcherSession);
  const showBackBtn = hasOverlay || (!!sessionContext && !isDispatcherSession);
  const contextTitle = hasOverlay ? overlayTitle : sessionTitle;
  const backTooltip = t('overlay.returnToAgenticOS');

  const handleContextBack = useCallback(() => {
    if (hasOverlay) {
      closeOverlay();
      return;
    }
    void openDispatcherSession({
      assistantWorkspace: sessionContext?.assistantWorkspace ?? null,
    });
  }, [hasOverlay, closeOverlay, sessionContext]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
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

  const handleDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      const target = e.target as HTMLElement | null;
      if (!target) return;
      if (target.closest(INTERACTIVE_SELECTOR)) return;
      onMaximize?.();
    },
    [onMaximize]
  );

  const rootCls = [
    'unified-top-bar',
    isMacOS && 'unified-top-bar--macos',
    hasWindowControls && 'unified-top-bar--has-controls',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <>
      <div
        className={rootCls}
        onMouseDown={handleMouseDown}
        onDoubleClick={handleDoubleClick}
        role="toolbar"
        aria-label={t('nav.aria.sceneHeader')}
      >
      {/* Left: app logo menu + overlay navigation (when an overlay is active) */}
      <div className="unified-top-bar__left">
        <div className="unified-top-bar__logo-wrap" ref={logoMenuAnchorRef}>
          <Tooltip content={t('header.openMenu')} placement="bottom" followCursor disabled={logoMenuOpen}>
            <button
              type="button"
              className={`unified-top-bar__logo-btn${logoMenuOpen ? ' is-open' : ''}`}
              aria-label={t('header.openMenu')}
              aria-haspopup="menu"
              aria-expanded={logoMenuOpen}
              onClick={() => setLogoMenuOpen((v) => !v)}
            >
              <img
                className="unified-top-bar__logo-img"
                src="/Logo-ICON.png"
                alt=""
                draggable={false}
              />
            </button>
          </Tooltip>

          {logoMenuOpen &&
            logoMenuPosition &&
            typeof document !== 'undefined' &&
            createPortal(
              <>
                <div
                  className="unified-top-bar__menu-backdrop"
                  aria-hidden="true"
                  onMouseDown={(e) => e.stopPropagation()}
                  onClick={closeLogoMenu}
                />
                <div
                  className="unified-top-bar__menu"
                  role="menu"
                  style={{ top: logoMenuPosition.top, left: logoMenuPosition.left }}
                  onMouseDown={(e) => e.stopPropagation()}
                >
                  <button
                    type="button"
                    className="unified-top-bar__menu-item"
                    role="menuitem"
                    onClick={handleFloatingMode}
                  >
                    <PictureInPicture2 size={14} aria-hidden="true" />
                    <span>{t('header.switchToToolbar')}</span>
                  </button>
                  <button
                    type="button"
                    className="unified-top-bar__menu-item"
                    role="menuitem"
                    onClick={handleLogoAbout}
                  >
                    <Info size={14} aria-hidden="true" />
                    <span>{t('header.about')}</span>
                  </button>
                </div>
              </>,
              document.body
            )}
        </div>

        {hasOverlay && (
          <Tooltip content={t('nav.sessionCapsule.openTaskList')} placement="bottom" followCursor>
            <button
              type="button"
              className="unified-top-bar__task-list-btn"
              onClick={openSessionListDialog}
              aria-label={t('nav.sessionCapsule.openTaskList')}
              data-testid="unified-top-bar-task-list"
            >
              <ListChecks size={14} strokeWidth={2.25} aria-hidden="true" />
            </button>
          </Tooltip>
        )}

        {showContextNav && (
          <div className="unified-top-bar__context-nav">
            {showBackBtn && (
              <Tooltip content={backTooltip} placement="bottom" followCursor>
                <button
                  type="button"
                  className="unified-top-bar__back-btn"
                  onClick={handleContextBack}
                  aria-label={backTooltip}
                  data-testid="unified-top-bar-back"
                >
                  <ArrowLeft size={14} strokeWidth={2.25} aria-hidden="true" />
                </button>
              </Tooltip>
            )}
            {contextTitle && (
              <div className="unified-top-bar__context-title">
                {!hasOverlay && sessionWorkspaceName && !isDispatcherSession && (
                  <span className="unified-top-bar__context-mode">
                    {sessionContext?.mode}
                  </span>
                )}
                {!hasOverlay && sessionWorkspaceName && !isDispatcherSession && (
                  <span className="unified-top-bar__context-sep" aria-hidden="true">/</span>
                )}
                {!hasOverlay && sessionWorkspaceName && !isDispatcherSession ? (
                  <span className="unified-top-bar__context-workspace">
                    <FolderOpen size={11} aria-hidden="true" />
                    <span>{sessionWorkspaceName}</span>
                  </span>
                ) : (
                  <span className="unified-top-bar__context-label">{contextTitle}</span>
                )}
              </div>
            )}
          </div>
        )}
      </div>

      {/* Center: global search — absolutely centered in the bar */}
      <div className="unified-top-bar__search">
        <Tooltip
          content={t('nav.search.headerSearchHint')}
          placement="bottom"
          followCursor
        >
          <button
            type="button"
            className="unified-top-bar__search-trigger"
            onClick={() => setSearchOpen(true)}
            aria-label={t('nav.search.headerSearchHint')}
          >
            <span className="unified-top-bar__search-row">
              <span className="unified-top-bar__search-icon" aria-hidden="true">
                <Search size={12} />
              </span>
              <span className="unified-top-bar__search-label">
                {t('nav.search.headerSearchHint')}
              </span>
            </span>
          </button>
        </Tooltip>
        <GlobalSearchDialog open={searchOpen} onClose={() => setSearchOpen(false)} />
      </div>

      {/* Right: remote control + window controls (notification bell: WorkspaceBody bottom-right) */}
      <div className="unified-top-bar__right">
        {isTauriDesktop && (
          <RemoteControlButton
            status={remoteConnectStatus}
            onOpenDialog={() => void handleRemoteConnect()}
            onStatusChange={setRemoteConnectStatus}
          />
        )}

        {hasWindowControls && !isMacOS && (
          <div className="unified-top-bar__controls">
            <WindowControls
              onMinimize={onMinimize!}
              onMaximize={onMaximize!}
              onClose={onClose!}
              isMaximized={isMaximized}
            />
          </div>
        )}
      </div>
      </div>

      <SessionListDialog />
      <RemoteConnectDialog isOpen={showRemoteConnect} onClose={() => setShowRemoteConnect(false)} />
      <Modal
        isOpen={showRemoteDisclaimer}
        onClose={() => setShowRemoteDisclaimer(false)}
        title={t('remoteConnect.disclaimerTitle')}
        showCloseButton
        size="large"
        contentInset
      >
        <RemoteConnectDisclaimerContent
          agreed={hasAgreedRemoteDisclaimer}
          onClose={() => setShowRemoteDisclaimer(false)}
          onAgree={handleAgreeDisclaimer}
        />
      </Modal>
    </>
  );
};

export default UnifiedTopBar;
