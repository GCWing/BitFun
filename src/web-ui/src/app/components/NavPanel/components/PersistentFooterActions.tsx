import React, { lazy, Suspense, useState, useCallback, useEffect } from 'react';
import {
  Settings,
  Info,
  MoreVertical,
  SquareTerminal,
  Terminal,
  Smartphone,
  Globe,
  ExternalLink,
  BarChart3,
  ChevronUp,
  MessageSquare,
  // LogIn,
} from 'lucide-react';
import { systemAPI } from '@/infrastructure/api';
import { Tooltip, Modal } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useSceneManager } from '../../../hooks/useSceneManager';
import { useNavSceneStore } from '../../../stores/navSceneStore';
import { useSceneStore } from '../../../stores/sceneStore';
import { useCanvasStore } from '@/app/components/panels/content-canvas/stores';
import { useNotification } from '@/shared/notification-system';
import { useAccountLoginState } from '@/infrastructure/account/useAccountLoginState';
// import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import NotificationButton from '../../TitleBar/NotificationButton';
import { usePrivacy } from '../../Privacy/PrivacyContext';
import {
  hasActionableUnreadReply,
  useFeedbackInboxStore,
} from '../../FeedbackDialog/feedbackInboxStore';
import {
  RemoteConnectDisclaimerContent,
} from '../../RemoteConnectDialog/RemoteConnectDisclaimer';
import {
  getRemoteConnectDisclaimerAgreed,
  setRemoteConnectDisclaimerAgreed,
} from '../../RemoteConnectDialog/remoteConnectDisclaimerStorage';

const RemoteConnectDialog = lazy(() => import('../../RemoteConnectDialog'));
// BitFun account login is disabled while the product feature is being retired.
// const AccountLoginDialog = lazy(() => import('../../AccountLoginDialog'));
const AboutDialog = lazy(() =>
  import('../../AboutDialog').then(module => ({ default: module.AboutDialog }))
);
const FeedbackDialog = lazy(() => import('../../FeedbackDialog'));

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
  const { warning } = useNotification();
  const { loggedIn: accountLoggedIn, deviceName: accountDeviceName } = useAccountLoginState();
  const { status: privacyStatus } = usePrivacy();
  const initializeFeedbackForMode = useFeedbackInboxStore(state => state.initializeForMode);
  const hasUnreadFeedback = useFeedbackInboxStore(state =>
    state.records.some(hasActionableUnreadReply),
  );
  const hasPrivacyUpdate = Boolean(
    privacyStatus?.enabled && privacyStatus.hasUnreadUpdate,
  );
  const hasMoreMenuAttention = hasUnreadFeedback || hasPrivacyUpdate;

  useEffect(() => {
    const onAutoExit = (event: Event) => {
      const detail = (event as CustomEvent<{ deviceName?: string; reason?: string }>).detail;
      const name = detail?.deviceName || 'peer';
      if (detail?.reason === 'peer_offline') {
        warning(t('accountLogin.peerAutoExitOffline', { name }));
      } else if (detail?.reason === 'rpc_failures') {
        warning(t('accountLogin.peerAutoExitRpc', { name }));
      }
    };
    window.addEventListener('peer-mode:auto-exit', onAutoExit);
    return () => window.removeEventListener('peer-mode:auto-exit', onAutoExit);
  }, [t, warning]);

  const [menuOpen, setMenuOpen] = useState(false);
  const [menuClosing, setMenuClosing] = useState(false);
  const [showAbout, setShowAbout] = useState(false);
  const [showFeedback, setShowFeedback] = useState(false);
  // const [showAccountLogin, setShowAccountLogin] = useState(false);
  const [showRemoteConnect, setShowRemoteConnect] = useState(false);
  const [remoteInitialGroup, setRemoteInitialGroup] = useState<'network' | 'bot' | 'account' | undefined>(undefined);
  const [showRemoteDisclaimer, setShowRemoteDisclaimer] = useState(false);
  const [feedbackPlatformEnabled, setFeedbackPlatformEnabled] = useState<boolean | null>(null);
  const [hasAgreedRemoteDisclaimer, setHasAgreedRemoteDisclaimer] = useState<boolean>(() => getRemoteConnectDisclaimerAgreed());

  useEffect(() => {
    let active = true;
    void systemAPI.getSystemInfo().then(info => {
      if (active) setFeedbackPlatformEnabled(info.platform === 'openharmony');
    }).catch(() => {
      if (active) setFeedbackPlatformEnabled(false);
    });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!feedbackPlatformEnabled || !privacyStatus) return;
    void initializeFeedbackForMode(privacyStatus.effectiveMode);
  }, [feedbackPlatformEnabled, initializeFeedbackForMode, privacyStatus]);

  // Account login retirement: do not reopen the disabled login dialog when a
  // stored account token expires.

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

  const handleOpenInsights = useCallback(() => {
    closeMenu();
    openScene('insights');
  }, [closeMenu, openScene]);

  const handleShowAbout = () => {
    closeMenu();
    setShowAbout(true);
  };

  const handleFeedback = useCallback(async () => {
    closeMenu();
    if (feedbackPlatformEnabled) {
      setShowFeedback(true);
      return;
    }
    try {
      const systemInfo = await systemAPI.getSystemInfo();
      if (systemInfo.platform === 'openharmony') {
        setFeedbackPlatformEnabled(true);
        setShowFeedback(true);
        return;
      }
    } catch {
      // Web and older desktop hosts retain the external feedback behavior.
    }
    await systemAPI.openExternal('https://gitcode.com/OpenHarmonyPCDeveloper/BitFun/issues');
  }, [closeMenu, feedbackPlatformEnabled]);

  // const handleAccountLogin = () => {
  //   closeMenu();
  //   setShowAccountLogin(true);
  // };

  const handleRemoteConnect = useCallback(async () => {
    closeMenu();

    if (hasAgreedRemoteDisclaimer || getRemoteConnectDisclaimerAgreed()) {
      setHasAgreedRemoteDisclaimer(true);
      setRemoteInitialGroup(undefined);
      setShowRemoteConnect(true);
      return;
    }

    setShowRemoteDisclaimer(true);
  }, [closeMenu, hasAgreedRemoteDisclaimer]);

  const handleAgreeDisclaimer = useCallback(() => {
    setRemoteConnectDisclaimerAgreed();
    setHasAgreedRemoteDisclaimer(true);
    setShowRemoteDisclaimer(false);
    setRemoteInitialGroup(undefined);
    setShowRemoteConnect(true);
  }, []);

  const isBrowserActive =
    activeTabId === 'browser' || (activeTabId === 'session' && isBrowserPanelActiveInCanvas);

  const SHOW_BROWSER_ENTRY = true;

  return (
    <>
      <div className="bitfun-nav-panel__footer">
        <div className="bitfun-nav-panel__footer-left">
          <div className="bitfun-nav-panel__footer-more-wrap">
            <Tooltip content={t('nav.moreOptions')} placement="right" followCursor disabled={menuOpen}>
              <button
                type="button"
                className={`bitfun-nav-panel__footer-btn bitfun-nav-panel__footer-btn--icon bitfun-nav-panel__footer-more-btn${menuOpen ? ' is-active' : ''}`}
                aria-label={hasMoreMenuAttention
                  ? t('header.moreOptionsAttention')
                  : t('nav.moreOptions')}
                aria-expanded={menuOpen}
                onClick={toggleMenu}
                data-testid="nav-footer-more-btn"
              >
                {menuOpen ? (
                  <MoreVertical size={15} aria-hidden="true" />
                ) : (
                  <span className="bitfun-nav-panel__footer-btn-icon-swap" aria-hidden="true">
                    <MoreVertical size={15} className="bitfun-nav-panel__footer-btn-icon-swap-default" />
                    <ChevronUp size={15} className="bitfun-nav-panel__footer-btn-icon-swap-hover" />
                  </span>
                )}
                {hasMoreMenuAttention ? (
                  <span className="bitfun-nav-panel__footer-more-unread" aria-hidden="true" />
                ) : null}
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
                  data-testid="nav-footer-menu"
                >
                  <button
                    type="button"
                    className="bitfun-nav-panel__footer-menu-item"
                    role="menuitem"
                    onClick={handleRemoteConnect}
                  >
                    <Smartphone size={14} />
                    <span className="bitfun-nav-panel__footer-menu-item-label">
                      {accountLoggedIn && accountDeviceName
                        ? accountDeviceName
                        : t('shared:features.remoteControl')}
                    </span>
                    {accountLoggedIn && (
                      <span className="bitfun-nav-panel__footer-menu-item-dot" />
                    )}
                  </button>
                  <div className="bitfun-nav-panel__footer-menu-divider" />
                  <button
                    type="button"
                    className="bitfun-nav-panel__footer-menu-item"
                    role="menuitem"
                    onClick={handleOpenInsights}
                  >
                    <BarChart3 size={14} />
                    <span>{t('scenes.insights')}</span>
                  </button>
                  <button
                    type="button"
                    className="bitfun-nav-panel__footer-menu-item"
                    role="menuitem"
                    onClick={handleOpenSettings}
                    data-testid="nav-footer-settings-item"
                  >
                    <Settings size={14} />
                    <span>{t('shared:features.settings')}</span>
                  </button>
                  <button
                    type="button"
                    className="bitfun-nav-panel__footer-menu-item"
                    role="menuitem"
                    onClick={handleFeedback}
                    aria-label={hasUnreadFeedback
                      ? t('feedback.inbox.entryUnread')
                      : t('header.feedback')}
                  >
                    <MessageSquare size={14} />
                    <span>{t('header.feedback')}</span>
                    {hasUnreadFeedback ? (
                      <span className="bitfun-nav-panel__footer-menu-unread" aria-hidden="true" />
                    ) : null}
                  </button>
                  <button
                      type="button"
                      className="bitfun-nav-panel__footer-menu-item"
                      role="menuitem"
                      onClick={handleShowAbout}
                      aria-label={hasPrivacyUpdate
                        ? t('privacy.aboutEntryUpdated')
                        : t('header.about')}
                  >
                      <Info size={14} />
                      <span>{t('header.about')}</span>
                      {hasPrivacyUpdate ? (
                        <span className="bitfun-nav-panel__footer-menu-unread" aria-hidden="true" />
                      ) : null}
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
              data-testid="shell-panel-entry"
            >
              <span className="bitfun-nav-panel__footer-btn-icon-swap" aria-hidden="true">
                <SquareTerminal size={15} className="bitfun-nav-panel__footer-btn-icon-swap-default" />
                <Terminal size={15} className="bitfun-nav-panel__footer-btn-icon-swap-hover" />
              </span>
            </button>
          </Tooltip>

          {SHOW_BROWSER_ENTRY && (
            <Tooltip content={t('scenes.browser')} placement="right">
              <button
                type="button"
                className={`bitfun-nav-panel__footer-btn bitfun-nav-panel__footer-btn--icon${isBrowserActive ? ' is-active' : ''}`}
                aria-label={t('scenes.browser')}
                aria-pressed={isBrowserActive}
                onClick={handleOpenBrowser}
                data-testid="browser-panel-entry"
              >
                <span className="bitfun-nav-panel__footer-btn-icon-swap" aria-hidden="true">
                  <Globe size={15} className="bitfun-nav-panel__footer-btn-icon-swap-default" />
                  <ExternalLink size={15} className="bitfun-nav-panel__footer-btn-icon-swap-hover" />
                </span>
              </button>
            </Tooltip>
          )}
        </div>

        <div className="bitfun-nav-panel__footer-right">
          <NotificationButton className="bitfun-nav-panel__footer-btn" navFooterHoverIconSwap />
        </div>
      </div>
      {showAbout && (
        <Suspense fallback={null}>
          <AboutDialog isOpen={showAbout} onClose={() => setShowAbout(false)} />
        </Suspense>
      )}
      {showFeedback && (
        <Suspense fallback={null}>
          <FeedbackDialog isOpen={showFeedback} onClose={() => setShowFeedback(false)} />
        </Suspense>
      )}
      {/* BitFun account login dialog is intentionally disabled. */}
      {showRemoteConnect && (
        <Suspense fallback={null}>
          <RemoteConnectDialog
            isOpen={showRemoteConnect}
            onClose={() => setShowRemoteConnect(false)}
            initialGroup={remoteInitialGroup}
          />
        </Suspense>
      )}
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
