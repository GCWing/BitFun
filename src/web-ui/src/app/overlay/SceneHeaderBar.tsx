/**
 * SceneHeaderBar — persistent top bar of the scene area.
 *
 * Replaces the old SceneBar tab strip. Always visible; layout (left → right):
 *
 *   [Home btn (overlay only)] [Overlay title (overlay only)] [Global search] [Notification] [WindowControls?]
 *
 * The empty space between search and edge acts as a drag region for moving the window.
 */

import React, { useCallback, useRef, useState } from 'react';
import { Home, Search } from 'lucide-react';
import { Tooltip, WindowControls } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useOverlayStore } from '../stores/overlayStore';
import { getOverlayDef } from './overlayRegistry';
import NotificationButton from '../components/TitleBar/NotificationButton';
import NavSearchDialog from '../components/NavPanel/NavSearchDialog';
import type { OverlaySceneId } from './types';
import { createLogger } from '@/shared/utils/logger';
import './SceneHeaderBar.scss';

const log = createLogger('SceneHeaderBar');

const INTERACTIVE_SELECTOR =
  'button, input, textarea, select, a, [role="button"], [contenteditable="true"], .window-controls';

interface SceneHeaderBarProps {
  activeOverlay: OverlaySceneId | null;
  onMinimize?: () => void;
  onMaximize?: () => void;
  onClose?: () => void;
  isMaximized?: boolean;
}

const SceneHeaderBar: React.FC<SceneHeaderBarProps> = ({
  activeOverlay,
  onMinimize,
  onMaximize,
  onClose,
  isMaximized = false,
}) => {
  const { t } = useI18n('common');
  const closeOverlay = useOverlayStore(s => s.closeOverlay);
  const hasWindowControls = !!(onMinimize && onMaximize && onClose);
  const hasOverlay = activeOverlay !== null;

  const [searchOpen, setSearchOpen] = useState(false);
  const lastMouseDownTimeRef = useRef<number>(0);

  const overlayDef = hasOverlay ? getOverlayDef(activeOverlay) : null;
  const overlayTitle = overlayDef?.labelKey ? t(overlayDef.labelKey) : (overlayDef?.label ?? '');

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

  const handleDoubleClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement | null;
    if (!target) return;
    if (target.closest(INTERACTIVE_SELECTOR)) return;
    onMaximize?.();
  }, [onMaximize]);

  return (
    <div
      className="scene-header-bar"
      onMouseDown={handleMouseDown}
      onDoubleClick={handleDoubleClick}
      role="toolbar"
      aria-label={t('nav.aria.sceneHeader')}
    >
      {/* Left: home button + overlay title (only when overlay active) */}
      {hasOverlay && (
        <div className="scene-header-bar__overlay-nav">
          <Tooltip content={t('overlay.returnToAgenticOS')} placement="bottom" followCursor>
            <button
              type="button"
              className="scene-header-bar__home-btn"
              onClick={closeOverlay}
              aria-label={t('overlay.returnToAgenticOS')}
            >
              <Home size={14} />
            </button>
          </Tooltip>
          {overlayTitle && (
            <span className="scene-header-bar__overlay-title">{overlayTitle}</span>
          )}
        </div>
      )}

      {/* Center: global search trigger */}
      <div className="scene-header-bar__search">
        <Tooltip content={t('nav.search.triggerTooltip')} placement="bottom" followCursor>
          <button
            type="button"
            className="scene-header-bar__search-trigger"
            onClick={() => setSearchOpen(true)}
            aria-label={t('nav.search.triggerTooltip')}
          >
            <span className="scene-header-bar__search-row">
              <span className="scene-header-bar__search-icon" aria-hidden="true">
                <Search size={12} />
              </span>
              <span className="scene-header-bar__search-label">
                {t('nav.search.triggerPlaceholder')}
              </span>
              <span className="scene-header-bar__search-shortcuts" aria-hidden="true">
                <kbd className="scene-header-bar__search-kbd">Alt+F</kbd>
              </span>
            </span>
          </button>
        </Tooltip>
        <NavSearchDialog open={searchOpen} onClose={() => setSearchOpen(false)} />
      </div>

      {/* Right: actions + window controls */}
      <div className="scene-header-bar__actions">
        <NotificationButton
          className="scene-header-bar__notification-btn"
          tooltipPlacement="bottom"
        />
      </div>

      {hasWindowControls && (
        <div className="scene-header-bar__controls">
          <WindowControls
            onMinimize={onMinimize!}
            onMaximize={onMaximize!}
            onClose={onClose!}
            isMaximized={isMaximized}
          />
        </div>
      )}
    </div>
  );
};

export default SceneHeaderBar;
