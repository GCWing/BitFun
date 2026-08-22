import React, { lazy, Suspense, useState, useCallback, useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';
import {
  Settings,
  Info,
  PictureInPicture2,
  Palette,
} from 'lucide-react';
import { Tooltip, Modal, PresenceBoundary } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useSceneStore } from '../../../stores/sceneStore';
import { activateProductAction } from '@/app/global-search/productActionActivator';
import { useToolbarModeContext } from '@/flow_chat/components/toolbar-mode/ToolbarModeContext';
import { useNotification } from '@/shared/notification-system';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import NotificationButton from '../../TitleBar/NotificationButton';
import GithubStarButton from './GithubStarButton';
import { RemoteConnectDisclaimerContent } from '../../RemoteConnectDialog/RemoteConnectDisclaimer';
import {
  getRemoteConnectDisclaimerAgreed,
  setRemoteConnectDisclaimerAgreed,
} from '../../RemoteConnectDialog/remoteConnectDisclaimerStorage';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import DeviceStatusControl from './DeviceStatusControl';

const RemoteConnectDialog = lazy(() => import('../../RemoteConnectDialog'));
const AboutDialog = lazy(() =>
  import('../../AboutDialog').then(module => ({ default: module.AboutDialog }))
);

const PersistentFooterActions: React.FC = () => {
  const { t } = useI18n('common');
  const activeTabId = useSceneStore((s) => s.activeTabId);
  const { enableToolbarMode } = useToolbarModeContext();
  const { warning } = useNotification();

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
  const [deviceOverviewOpen, setDeviceOverviewOpen] = useState(false);
  const menuTriggerRef = useRef<HTMLButtonElement>(null);
  const menuPopoverRef = useRef<HTMLDivElement>(null);
  const menuLayout = useAnchoredPopoverPosition({
    open: menuOpen,
    anchorRef: menuTriggerRef,
    popoverRef: menuPopoverRef,
    preferredPlacement: 'top',
    alignment: 'end',
    gap: 6,
  });
  const [showAbout, setShowAbout] = useState(false);
  const [showRemoteConnect, setShowRemoteConnect] = useState(false);
  const [remoteInitialGroup, setRemoteInitialGroup] = useState<'network' | 'bot' | 'account' | undefined>(undefined);
  const [showRemoteDisclaimer, setShowRemoteDisclaimer] = useState(false);
  const [hasAgreedRemoteDisclaimer, setHasAgreedRemoteDisclaimer] = useState<boolean>(
    () => getRemoteConnectDisclaimerAgreed(),
  );

  // Periodic token-expiry check. Only auto-open the dialog if the token has
  // actually expired while the app is running — not on startup. Lands on the
  // account group so the user can sign in again right away.
  useEffect(() => {
    const expiryCheck = setInterval(() => {
      remoteConnectAPI.accountTokenExpired().then((expired) => {
        if (expired) {
          setRemoteInitialGroup('account');
          setShowRemoteConnect(true);
        }
      });
    }, 60000);
    return () => clearInterval(expiryCheck);
  }, []);

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
      setDeviceOverviewOpen(false);
      setMenuOpen(true);
    }
  };

  const handleDeviceOverviewOpenChange = useCallback((nextOpen: boolean) => {
    if (nextOpen && menuOpen) {
      closeMenu();
    }
    setDeviceOverviewOpen(nextOpen);
  }, [closeMenu, menuOpen]);

  const handleOpenSettings = useCallback(() => {
    closeMenu();
    void activateProductAction('settings.open');
  }, [closeMenu]);

  const handleOpenThemeConfiguration = useCallback(() => {
    closeMenu();
    useSettingsStore.getState().openPage('application.appearance');
    void activateProductAction('settings.open');
  }, [closeMenu]);

  const handleShowAbout = () => {
    closeMenu();
    setShowAbout(true);
  };

  const handleFloatingMode = () => {
    closeMenu();
    enableToolbarMode();
  };

  const handleRemoteConnect = useCallback(() => {
    if (hasAgreedRemoteDisclaimer || getRemoteConnectDisclaimerAgreed()) {
      setHasAgreedRemoteDisclaimer(true);
      setRemoteInitialGroup(undefined);
      setShowRemoteConnect(true);
      return;
    }

    setRemoteInitialGroup(undefined);
    setShowRemoteDisclaimer(true);
  }, [hasAgreedRemoteDisclaimer]);

  useEffect(() => {
    const handlePlaybookOpen = (event: Event) => {
      const requestedGroup = (event as CustomEvent<{ group?: 'network' | 'bot' | 'account' }>).detail?.group;
      setRemoteInitialGroup(requestedGroup);
      if (hasAgreedRemoteDisclaimer || getRemoteConnectDisclaimerAgreed()) {
        setHasAgreedRemoteDisclaimer(true);
        setShowRemoteConnect(true);
      } else {
        setShowRemoteDisclaimer(true);
      }
    };
    window.addEventListener('bitfun:open-remote-connect', handlePlaybookOpen);
    return () => window.removeEventListener('bitfun:open-remote-connect', handlePlaybookOpen);
  }, [hasAgreedRemoteDisclaimer]);

  const handleAgreeDisclaimer = useCallback(() => {
    setRemoteConnectDisclaimerAgreed();
    setHasAgreedRemoteDisclaimer(true);
    setShowRemoteDisclaimer(false);
    setShowRemoteConnect(true);
  }, []);

  const isSettingsActive = activeTabId === 'settings';

  return (
    <>
      <div className="bitfun-nav-panel__footer" data-bf-component="nav-panel" data-bf-part="footer">
        <div className="bitfun-nav-panel__footer-left">
          <DeviceStatusControl
            open={deviceOverviewOpen}
            onOpenChange={handleDeviceOverviewOpenChange}
            onManageDevices={handleRemoteConnect}
          />
        </div>

        <div className="bitfun-nav-panel__footer-right">
          <GithubStarButton />
          <div className="bitfun-nav-panel__footer-menu-wrap">
            <Tooltip
              content={t('shared:features.settings')}
              placement="right"
              followCursor
              disabled={menuOpen}
            >
              <button
                ref={menuTriggerRef}
                type="button"
                className={`bitfun-nav-panel__footer-btn bitfun-nav-panel__footer-btn--icon${menuOpen || isSettingsActive ? ' is-active' : ''}`}
                aria-label={t('shared:features.settings')}
                aria-expanded={menuOpen}
                aria-haspopup="menu"
                aria-pressed={isSettingsActive}
                onClick={toggleMenu}
                data-testid="nav-footer-settings-item"
                data-bf-component="nav-panel"
                data-bf-part="settingsEntry"
                data-bf-state={menuOpen ? 'open' : isSettingsActive ? 'active' : undefined}
              >
                <Settings size={15} aria-hidden="true" />
              </button>
            </Tooltip>

            {menuOpen && createPortal(
              <>
                <div
                  className="bitfun-nav-panel__footer-backdrop"
                  onClick={closeMenu}
                />
                <div
                  ref={menuPopoverRef}
                  className={`bitfun-nav-panel__footer-menu${menuClosing ? ' is-closing' : ''}`}
                  role="menu"
                  aria-label={t('shared:features.settings')}
                  data-testid="nav-settings-menu"
                  data-bf-component="nav-panel"
                  data-bf-part="footerMenu"
                  data-bf-state={menuClosing ? 'closing' : 'open'}
                  data-bf-placement={menuLayout?.placement ?? 'top'}
                  style={{
                    top: `${menuLayout?.top ?? 0}px`,
                    left: `${menuLayout?.left ?? 0}px`,
                    visibility: menuLayout ? 'visible' : 'hidden',
                  }}
                >
                  <button
                    type="button"
                    className="bitfun-nav-panel__footer-menu-item"
                    role="menuitem"
                    onClick={handleFloatingMode}
                    data-testid="nav-settings-floating-item"
                    data-bf-component="nav-panel"
                    data-bf-part="footerMenuItem"
                    data-bf-action="floating-window"
                  >
                    <PictureInPicture2 size={14} aria-hidden="true" />
                    <span>{t('nav.settingsMenu.floatingWindow')}</span>
                  </button>
                  <NotificationButton menuItem onActivate={closeMenu} />
                  <button
                    type="button"
                    className="bitfun-nav-panel__footer-menu-item"
                    role="menuitem"
                    onClick={handleOpenThemeConfiguration}
                    data-testid="nav-settings-theme-item"
                    data-bf-component="nav-panel"
                    data-bf-part="footerMenuItem"
                    data-bf-action="appearance-configuration"
                  >
                    <Palette size={14} aria-hidden="true" />
                    <span>{t('nav.settingsMenu.themeConfiguration')}</span>
                  </button>
                  <div className="bitfun-nav-panel__footer-menu-divider" data-bf-component="nav-panel" data-bf-part="footerMenuDivider" />
                  <button
                    type="button"
                    className="bitfun-nav-panel__footer-menu-item"
                    role="menuitem"
                    onClick={handleOpenSettings}
                    data-testid="nav-settings-open-item"
                    data-bf-component="nav-panel"
                    data-bf-part="footerMenuItem"
                    data-bf-action="open-settings"
                  >
                    <Settings size={14} aria-hidden="true" />
                    <span>{t('nav.settingsMenu.openSettings')}</span>
                  </button>
                  <button
                    type="button"
                    className="bitfun-nav-panel__footer-menu-item"
                    role="menuitem"
                    onClick={handleShowAbout}
                    data-testid="nav-settings-about-item"
                    data-bf-component="nav-panel"
                    data-bf-part="footerMenuItem"
                    data-bf-action="about"
                  >
                    <Info size={14} aria-hidden="true" />
                    <span>{t('nav.settingsMenu.about')}</span>
                  </button>
                </div>
              </>,
              getAppearanceOverlayHost(),
            )}
          </div>
        </div>
      </div>
      <PresenceBoundary active={showAbout}>
        <Suspense fallback={null}>
          <AboutDialog isOpen={showAbout} onClose={() => setShowAbout(false)} />
        </Suspense>
      </PresenceBoundary>
      <PresenceBoundary active={showRemoteConnect}>
        <Suspense fallback={null}>
          <RemoteConnectDialog
            isOpen={showRemoteConnect}
            onClose={() => setShowRemoteConnect(false)}
            initialGroup={remoteInitialGroup}
          />
        </Suspense>
      </PresenceBoundary>
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
