import React, { useState, useCallback } from 'react';
import { Settings, Info, MoreVertical, PictureInPicture2, Wifi } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useSceneManager } from '../../../hooks/useSceneManager';
import { useToolbarModeContext } from '@/flow_chat/components/toolbar-mode/ToolbarModeContext';
import NotificationButton from '../../TitleBar/NotificationButton';
import { AboutDialog } from '../../AboutDialog';
import { RemoteConnectDialog } from '../../RemoteConnectDialog';

const PersistentFooterActions: React.FC = () => {
  const { t } = useI18n('common');
  const { openScene } = useSceneManager();
  const { enableToolbarMode } = useToolbarModeContext();

  const [menuOpen, setMenuOpen] = useState(false);
  const [menuClosing, setMenuClosing] = useState(false);
  const [showAbout, setShowAbout] = useState(false);
  const [showRemoteConnect, setShowRemoteConnect] = useState(false);

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

  const handleShowAbout = () => {
    closeMenu();
    setShowAbout(true);
  };

  const handleFloatingMode = () => {
    closeMenu();
    enableToolbarMode();
  };

  const handleRemoteConnect = () => {
    closeMenu();
    setShowRemoteConnect(true);
  };

  return (
    <div className="bitfun-nav-panel__footer">
      <div className="bitfun-nav-panel__footer-more-wrap">
        <Tooltip content={t('nav.moreOptions')} placement="right" followCursor disabled={menuOpen}>
          <button
            type="button"
            className={`bitfun-nav-panel__footer-btn bitfun-nav-panel__footer-btn--icon${menuOpen ? ' is-active' : ''}`}
            aria-label={t('nav.moreOptions')}
            aria-expanded={menuOpen}
            onClick={toggleMenu}
          >
            <MoreVertical size={15} />
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
              <button
                type="button"
                className="bitfun-nav-panel__footer-menu-item"
                role="menuitem"
                onClick={handleRemoteConnect}
              >
                <Wifi size={14} />
                <span>{t('header.remoteConnect')}</span>
              </button>
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

      <NotificationButton className="bitfun-nav-panel__footer-btn" />
      <AboutDialog isOpen={showAbout} onClose={() => setShowAbout(false)} />
      <RemoteConnectDialog isOpen={showRemoteConnect} onClose={() => setShowRemoteConnect(false)} />
    </div>
  );
};

export default PersistentFooterActions;
