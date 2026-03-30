import React, { useState, useCallback, useRef } from 'react';
import {
  Settings,
  Info,
  MoreVertical,
  PictureInPicture2,
  SquareTerminal,
  Terminal,
  Smartphone,
  Globe,
  Network,
  Layers,
  PanelsTopLeft,
  BarChart3,
  LineChart,
  ChevronUp,
} from 'lucide-react';
import { Tooltip, Modal } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useSceneManager } from '../../../hooks/useSceneManager';
import { useNavSceneStore } from '../../../stores/navSceneStore';
import { useSceneStore } from '../../../stores/sceneStore';
import { useCanvasStore } from '@/app/components/panels/content-canvas/stores';
import { useToolbarModeContext } from '@/flow_chat/components/toolbar-mode/ToolbarModeContext';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useNotification } from '@/shared/notification-system';
import NotificationButton from '../../TitleBar/NotificationButton';
import { AboutDialog } from '../../AboutDialog';
import { RemoteConnectDialog } from '../../RemoteConnectDialog';
import {
  RemoteConnectDisclaimerContent,
} from '../../RemoteConnectDialog/RemoteConnectDisclaimer';
import {
  getRemoteConnectDisclaimerAgreed,
  setRemoteConnectDisclaimerAgreed,
} from '../../RemoteConnectDialog/remoteConnectDisclaimerStorage';
import { MERMAID_INTERACTIVE_EXAMPLE } from '@/flow_chat/constants/mermaidExamples';

const PersistentFooterActions: React.FC = () => {
  const { t } = useI18n('common');
  const { openScene } = useSceneManager();
  const activeTabId = useSceneStore((s) => s.activeTabId);
  const showSceneNav = useNavSceneStore((s) => s.showSceneNav);
  const navSceneId = useNavSceneStore((s) => s.navSceneId);
  const openNavScene = useNavSceneStore((s) => s.openNavScene);
  const closeNavScene = useNavSceneStore((s) => s.closeNavScene);

  // Check if a browser panel is the active tab in the AuxPane canvas
  const isBrowserPanelActiveInCanvas = useCanvasStore((s) => {
    const activeTab = s.primaryGroup.tabs.find((t) => t.id === s.primaryGroup.activeTabId);
    return activeTab?.content.type === 'browser';
  });
  const isMermaidPanelActiveInCanvas = useCanvasStore((s) => {
    const activeTab = s.primaryGroup.tabs.find((t) => t.id === s.primaryGroup.activeTabId);
    return activeTab?.content.type === 'mermaid-editor';
  });
  const { enableToolbarMode } = useToolbarModeContext();
  const { hasWorkspace } = useCurrentWorkspace();
  const { warning } = useNotification();

  const [menuOpen, setMenuOpen] = useState(false);
  const [menuClosing, setMenuClosing] = useState(false);
  const [multimodalOpen, setMultimodalOpen] = useState(false);
  const multimodalHoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [showAbout, setShowAbout] = useState(false);
  const [showRemoteConnect, setShowRemoteConnect] = useState(false);
  const [showRemoteDisclaimer, setShowRemoteDisclaimer] = useState(false);
  const [hasAgreedRemoteDisclaimer, setHasAgreedRemoteDisclaimer] = useState<boolean>(() => getRemoteConnectDisclaimerAgreed());

  const closeMenu = useCallback(() => {
    setMenuClosing(true);
    setTimeout(() => {
      setMenuOpen(false);
      setMenuClosing(false);
    }, 150);
  }, []);

  const toggleMenu = () => {
    if (menuOpen) {
      closeMenu();
    } else {
      setMenuOpen(true);
    }
  };

  const handleOpenSettings = () => {
    closeMenu();
    openScene('settings');
  };

  const handleOpenShell = useCallback(() => {
    if (showSceneNav && navSceneId === 'shell') {
      closeNavScene();
      return;
    }
    openNavScene('shell');
  }, [closeNavScene, navSceneId, openNavScene, showSceneNav]);

  const handleOpenBrowser = useCallback(() => {
    if (activeTabId === 'session') {
      // Open browser as a panel in the AuxPane (right side of chat)
      window.dispatchEvent(new CustomEvent('agent-create-tab', {
        detail: {
          type: 'browser',
          title: t('scenes.browser'),
          checkDuplicate: true,
          duplicateCheckKey: 'browser-panel',
          replaceExisting: false,
        },
      }));
    } else {
      openScene('browser');
    }
  }, [activeTabId, openScene, t]);

  const handleOpenMermaidEditor = useCallback(() => {
    const title = t('scenes.mermaidEditor');
    const detail = {
      type: 'mermaid-editor' as const,
      title,
      data: { ...MERMAID_INTERACTIVE_EXAMPLE, title },
      metadata: {
        duplicateCheckKey: 'mermaid-dual-mode-demo',
      },
      checkDuplicate: true,
      duplicateCheckKey: 'mermaid-dual-mode-demo',
      replaceExisting: false,
    };

    if (activeTabId === 'session') {
      window.dispatchEvent(new CustomEvent('agent-create-tab', { detail }));
    } else {
      openScene('mermaid');
    }
  }, [activeTabId, openScene, t]);

  const handleMultimodalEnter = useCallback(() => {
    if (multimodalHoverTimerRef.current) clearTimeout(multimodalHoverTimerRef.current);
    multimodalHoverTimerRef.current = setTimeout(() => setMultimodalOpen(true), 100);
  }, []);

  const handleMultimodalLeave = useCallback(() => {
    if (multimodalHoverTimerRef.current) clearTimeout(multimodalHoverTimerRef.current);
    multimodalHoverTimerRef.current = setTimeout(() => setMultimodalOpen(false), 180);
  }, []);

  const handleOpenInsights = useCallback(() => {
    openScene('insights');
  }, [openScene]);

  const insightsTooltip = t('nav.items.insights');
  const isInsightsActive = activeTabId === 'insights';

  const handleShowAbout = () => {
    closeMenu();
    setShowAbout(true);
  };

  const handleFloatingMode = () => {
    closeMenu();
    enableToolbarMode();
  };

  const handleRemoteConnect = useCallback(async () => {
    if (!hasWorkspace) {
      warning(t('header.remoteConnectRequiresWorkspace'));
      return;
    }

    closeMenu();

    if (hasAgreedRemoteDisclaimer || getRemoteConnectDisclaimerAgreed()) {
      setHasAgreedRemoteDisclaimer(true);
      setShowRemoteConnect(true);
      return;
    }

    setShowRemoteDisclaimer(true);
  }, [hasWorkspace, warning, t, closeMenu, hasAgreedRemoteDisclaimer]);

  const handleAgreeDisclaimer = useCallback(() => {
    setRemoteConnectDisclaimerAgreed();
    setHasAgreedRemoteDisclaimer(true);
    setShowRemoteDisclaimer(false);
    setShowRemoteConnect(true);
  }, []);

  return (
    <>
      <div className="bitfun-nav-panel__footer">
        <div className="bitfun-nav-panel__footer-left">
          <div className="bitfun-nav-panel__footer-more-wrap">
            <Tooltip content={t('nav.moreOptions')} placement="right" followCursor disabled={menuOpen}>
              <button
                type="button"
                className={`bitfun-nav-panel__footer-btn bitfun-nav-panel__footer-btn--icon${menuOpen ? ' is-active' : ''}`}
                aria-label={t('nav.moreOptions')}
                aria-expanded={menuOpen}
                onClick={toggleMenu}
              >
                {menuOpen ? (
                  <MoreVertical size={15} aria-hidden="true" />
                ) : (
                  <span className="bitfun-nav-panel__footer-btn-icon-swap" aria-hidden="true">
                    <MoreVertical size={15} className="bitfun-nav-panel__footer-btn-icon-swap-default" />
                    <ChevronUp size={15} className="bitfun-nav-panel__footer-btn-icon-swap-hover" />
                  </span>
                )}
              </button>
            </Tooltip>

            {menuOpen && (
              <>
                <div
                  className="bitfun-nav-panel__footer-backdrop"
                  onClick={closeMenu}
                />
                <div
                  className={`bitfun-nav-panel__footer-menu${menuClosing ? ' is-closing' : ''}`}
                  role="menu"
                >
                  <Tooltip
                    content={t('header.remoteConnectRequiresWorkspace')}
                    placement="right"
                    disabled={hasWorkspace}
                  >
                    <button
                      type="button"
                      className={`bitfun-nav-panel__footer-menu-item${!hasWorkspace ? ' is-disabled' : ''}`}
                      role="menuitem"
                      aria-disabled={!hasWorkspace}
                      onClick={handleRemoteConnect}
                    >
                      <Smartphone size={14} />
                      <span>{t('header.remoteConnect')}</span>
                    </button>
                  </Tooltip>
                  <div className="bitfun-nav-panel__footer-menu-divider" />
                  <button
                    type="button"
                    className="bitfun-nav-panel__footer-menu-item"
                    role="menuitem"
                    onClick={handleFloatingMode}
                  >
                    <PictureInPicture2 size={14} />
                    <span>{t('header.switchToToolbar')}</span>
                  </button>
                  <div className="bitfun-nav-panel__footer-menu-divider" />
                  <button
                    type="button"
                    className="bitfun-nav-panel__footer-menu-item"
                    role="menuitem"
                    onClick={handleOpenSettings}
                  >
                    <Settings size={14} />
                    <span>{t('tabs.settings')}</span>
                  </button>
                  <button
                    type="button"
                    className="bitfun-nav-panel__footer-menu-item"
                    role="menuitem"
                    onClick={handleShowAbout}
                  >
                    <Info size={14} />
                    <span>{t('header.about')}</span>
                  </button>
                </div>
              </>
            )}
          </div>

          <Tooltip content={t('scenes.shell')} placement="right">
            <button
              type="button"
              className={`bitfun-nav-panel__footer-btn bitfun-nav-panel__footer-btn--icon${showSceneNav && navSceneId === 'shell' ? ' is-active' : ''}`}
              aria-label={t('scenes.shell')}
              aria-pressed={showSceneNav && navSceneId === 'shell'}
              onClick={handleOpenShell}
            >
              <span className="bitfun-nav-panel__footer-btn-icon-swap" aria-hidden="true">
                <SquareTerminal size={15} className="bitfun-nav-panel__footer-btn-icon-swap-default" />
                <Terminal size={15} className="bitfun-nav-panel__footer-btn-icon-swap-hover" />
              </span>
            </button>
          </Tooltip>

        <div
          className="bitfun-nav-panel__footer-multimodal-wrap"
          onMouseEnter={handleMultimodalEnter}
          onMouseLeave={handleMultimodalLeave}
        >
          {(() => {
            const isBrowserActive = activeTabId === 'browser' || (activeTabId === 'session' && isBrowserPanelActiveInCanvas);
            const isMermaidActive = activeTabId === 'mermaid' || (activeTabId === 'session' && isMermaidPanelActiveInCanvas);
            const isAnyActive = isBrowserActive || isMermaidActive;
            return (
              <>
                <button
                  type="button"
                  className={`bitfun-nav-panel__footer-btn bitfun-nav-panel__footer-btn--icon${isAnyActive ? ' is-active' : ''}${multimodalOpen ? ' is-hover-open' : ''}`}
                  aria-label={t('nav.multimodalTools')}
                  aria-expanded={multimodalOpen}
                  aria-haspopup="menu"
                >
                  <span className="bitfun-nav-panel__footer-btn-icon-swap" aria-hidden="true">
                    <Layers size={15} className="bitfun-nav-panel__footer-btn-icon-swap-default" />
                    <PanelsTopLeft size={15} className="bitfun-nav-panel__footer-btn-icon-swap-hover" />
                  </span>
                </button>

                {multimodalOpen && (
                  <div
                    className="bitfun-nav-panel__footer-multimodal-menu"
                    role="menu"
                    aria-label={t('nav.multimodalTools')}
                  >
                    <button
                      type="button"
                      className={`bitfun-nav-panel__footer-multimodal-item${isBrowserActive ? ' is-active' : ''}`}
                      role="menuitem"
                      aria-pressed={isBrowserActive}
                      onClick={handleOpenBrowser}
                    >
                      <Globe size={13} className="bitfun-nav-panel__footer-multimodal-item-icon" />
                      <span className="bitfun-nav-panel__footer-multimodal-item-label">{t('scenes.browser')}</span>
                    </button>

                    <button
                      type="button"
                      className={`bitfun-nav-panel__footer-multimodal-item${isMermaidActive ? ' is-active' : ''}`}
                      role="menuitem"
                      aria-pressed={isMermaidActive}
                      onClick={handleOpenMermaidEditor}
                    >
                      <Network size={13} className="bitfun-nav-panel__footer-multimodal-item-icon" />
                      <span className="bitfun-nav-panel__footer-multimodal-item-label">{t('scenes.mermaidEditor')}</span>
                    </button>
                  </div>
                )}
              </>
            );
          })()}
        </div>

          <Tooltip content={insightsTooltip} placement="right" followCursor>
            <button
              type="button"
              className={`bitfun-nav-panel__footer-btn bitfun-nav-panel__footer-btn--icon${isInsightsActive ? ' is-active' : ''}`}
              onClick={handleOpenInsights}
              aria-label={insightsTooltip}
            >
              <span className="bitfun-nav-panel__footer-btn-icon-swap" aria-hidden="true">
                <BarChart3 size={15} className="bitfun-nav-panel__footer-btn-icon-swap-default" />
                <LineChart size={15} className="bitfun-nav-panel__footer-btn-icon-swap-hover" />
              </span>
            </button>
          </Tooltip>
        </div>

        <div className="bitfun-nav-panel__footer-right">
          <NotificationButton className="bitfun-nav-panel__footer-btn" navFooterHoverIconSwap />
        </div>
      </div>
      <AboutDialog isOpen={showAbout} onClose={() => setShowAbout(false)} />
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

export default PersistentFooterActions;
