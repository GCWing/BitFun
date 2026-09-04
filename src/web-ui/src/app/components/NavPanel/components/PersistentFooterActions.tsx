import React, { lazy, Suspense, useState, useCallback, useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';

import {
  Icon,
  IconButton,
  Menu,
  MenuItem,
  MenuSeparator,
  Tooltip,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@openbitfun/ui';
import { RetainedMountBoundary } from '@/shared/presence';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useSceneStore } from '../../../stores/sceneStore';
import { activateProductAction } from '@/app/global-search/productActionActivator';
import { useToolbarModeContext } from '@/flow_chat/components/toolbar-mode/ToolbarModeContext';
import { useNotification } from '@/shared/notification-system';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import NotificationButton from '../../TitleBar/NotificationButton';
import { RemoteConnectDisclaimerContent } from '../../RemoteConnectDialog/RemoteConnectDisclaimer';
import {
  getRemoteConnectDisclaimerAgreed,
  setRemoteConnectDisclaimerAgreed,
} from '../../RemoteConnectDialog/remoteConnectDisclaimerStorage';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import DeviceStatusControl from './DeviceStatusControl';
import AppearanceQuickSwitchMenuItem from './AppearanceQuickSwitchMenuItem';

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
  const [appearanceSubmenuOpen, setAppearanceSubmenuOpen] = useState(false);
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
    setAppearanceSubmenuOpen(false);
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
      setAppearanceSubmenuOpen(false);
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

  const handleOpenAppearanceSettings = useCallback(() => {
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
    window.addEventListener('openbitfun:open-remote-connect', handlePlaybookOpen);
    return () => window.removeEventListener('openbitfun:open-remote-connect', handlePlaybookOpen);
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
      <div className="openbitfun-nav-panel__footer" data-openbitfun-component="nav-panel" data-openbitfun-part="footer">
        <div className="openbitfun-nav-panel__footer-left">
          <DeviceStatusControl
            open={deviceOverviewOpen}
            onOpenChange={handleDeviceOverviewOpenChange}
            onManageDevices={handleRemoteConnect}
          />
        </div>

        <div className="openbitfun-nav-panel__footer-right">
          <div className="openbitfun-nav-panel__footer-menu-wrap">
            <Tooltip
              content={t('shared:features.settings')}
              placement="right"
              followCursor
              disabled={menuOpen}
            >
              <IconButton
                ref={menuTriggerRef}
                className={`openbitfun-nav-panel__footer-btn openbitfun-nav-panel__footer-btn--icon${menuOpen || isSettingsActive ? ' is-active' : ''}`}
                aria-label={t('shared:features.settings')}
                aria-expanded={menuOpen}
                aria-haspopup="menu"
                aria-pressed={isSettingsActive}
                onClick={toggleMenu}
                data-testid="nav-footer-settings-item"
                data-openbitfun-component="nav-panel"
                data-openbitfun-part="settingsEntry"
                data-openbitfun-state={menuOpen ? 'open' : isSettingsActive ? 'active' : undefined}
                icon={<Icon name="gear" size="sm" aria-hidden="true" />}
                size="sm"
                variant="quiet"
              />
            </Tooltip>

            {menuOpen && createPortal(
              <>
                <div
                  className="openbitfun-nav-panel__footer-backdrop"
                  onClick={closeMenu}
                />
                <Menu
                  ref={menuPopoverRef}
                  className={`openbitfun-nav-panel__footer-menu${menuClosing ? ' is-closing' : ''}`}
                  aria-label={t('shared:features.settings')}
                  data-testid="nav-settings-menu"
                  onKeyDown={(event) => {
                    if (event.key !== 'Escape') return;
                    event.preventDefault();
                    if (appearanceSubmenuOpen) {
                      setAppearanceSubmenuOpen(false);
                    } else {
                      closeMenu();
                      menuTriggerRef.current?.focus();
                    }
                  }}
                  style={{
                    top: `${menuLayout?.top ?? 0}px`,
                    left: `${menuLayout?.left ?? 0}px`,
                    visibility: menuLayout ? 'visible' : 'hidden',
                  }}
                >
                  <MenuItem
                    leading={<Icon name="floating-window" size="sm" aria-hidden="true" />}
                    onClick={handleFloatingMode}
                    data-testid="nav-settings-floating-item"
                  >
                    {t('nav.settingsMenu.floatingWindow')}
                  </MenuItem>
                  <NotificationButton menuItem onActivate={closeMenu} />
                  <AppearanceQuickSwitchMenuItem
                    open={appearanceSubmenuOpen}
                    onOpenChange={setAppearanceSubmenuOpen}
                    onCloseParentMenu={closeMenu}
                    onOpenAppearanceSettings={handleOpenAppearanceSettings}
                  />
                  <MenuSeparator />
                  <MenuItem
                    leading={<Icon name="gear" size="sm" aria-hidden="true" />}
                    onClick={handleOpenSettings}
                    data-testid="nav-settings-open-item"
                  >
                    {t('nav.settingsMenu.openSettings')}
                  </MenuItem>
                  <MenuItem
                    leading={<Icon name="info" size="sm" aria-hidden="true" />}
                    onClick={handleShowAbout}
                    data-testid="nav-settings-about-item"
                  >
                    {t('nav.settingsMenu.about')}
                  </MenuItem>
                </Menu>
              </>,
              getAppearanceOverlayHost(),
            )}
          </div>
        </div>
      </div>
      <RetainedMountBoundary present={showAbout}>
        <Suspense fallback={null}>
          <AboutDialog isOpen={showAbout} onClose={() => setShowAbout(false)} />
        </Suspense>
      </RetainedMountBoundary>
      <RetainedMountBoundary present={showRemoteConnect}>
        <Suspense fallback={null}>
          <RemoteConnectDialog
            isOpen={showRemoteConnect}
            onClose={() => setShowRemoteConnect(false)}
            initialGroup={remoteInitialGroup}
          />
        </Suspense>
      </RetainedMountBoundary>
      <Dialog
        open={showRemoteDisclaimer}
        onOpenChange={(nextOpen) => { if (!nextOpen) setShowRemoteDisclaimer(false); }}
        size="lg"
      >
        <DialogHeader>
          <DialogHeading>
            <DialogTitle>{t('remoteConnect.disclaimerTitle')}</DialogTitle>
          </DialogHeading>
          <DialogClose />
        </DialogHeader>
        <DialogBody>
        <RemoteConnectDisclaimerContent
          agreed={hasAgreedRemoteDisclaimer}
          onClose={() => setShowRemoteDisclaimer(false)}
          onAgree={handleAgreeDisclaimer}
        />
              </DialogBody>
      </Dialog>
    </>
  );
};

export default PersistentFooterActions;
