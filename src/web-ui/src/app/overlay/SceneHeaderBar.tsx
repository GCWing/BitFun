/**
 * SceneHeaderBar — DEPRECATED.
 *
 * This component has been superseded by UnifiedTopBar
 * (src/web-ui/src/app/components/UnifiedTopBar/UnifiedTopBar.tsx).
 *
 * UnifiedTopBar is a full-width bar that spans the entire window and combines
 * the functionality of the former NavBar (left column) and SceneHeaderBar (scene area).
 *
 * This file is kept as a reference but is no longer imported or rendered.
 * It can be safely deleted in a future cleanup pass.
 */

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Home, Search } from 'lucide-react';
import { Tooltip, WindowControls } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useOverlayStore } from '../stores/overlayStore';
import { getOverlayDef } from './overlayRegistry';
import NotificationButton from '../components/TitleBar/NotificationButton';
import GlobalSearchDialog from '../components/GlobalSearchDialog/GlobalSearchDialog';
import type { OverlaySceneId } from './types';
import { createLogger } from '@/shared/utils/logger';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { ALL_SHORTCUTS } from '@/shared/constants/shortcuts';
import './SceneHeaderBar.scss';

const log = createLogger('SceneHeaderBar');

const NAV_TOGGLE_SEARCH_DEF = ALL_SHORTCUTS.find((d) => d.id === 'nav.toggleSearch')!;

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

  const toggleNavSearch = useCallback(() => {
    setSearchOpen((v) => !v);
  }, []);

  useShortcut(
    NAV_TOGGLE_SEARCH_DEF.id,
    NAV_TOGGLE_SEARCH_DEF.config,
    toggleNavSearch,
    { priority: 5, description: NAV_TOGGLE_SEARCH_DEF.descriptionKey }
  );

  // Secondary binding (not listed separately in keyboard settings — same action as Mod+K)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        !e.altKey ||
        e.ctrlKey ||
        e.metaKey ||
        e.shiftKey ||
        e.key.toLowerCase() !== 'f'
      ) {
        return;
      }
      e.preventDefault();
      toggleNavSearch();
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [toggleNavSearch]);

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
        <Tooltip content={t('nav.search.headerSearchHint')} placement="bottom" followCursor>
          <button
            type="button"
            className="scene-header-bar__search-trigger"
            onClick={() => setSearchOpen(true)}
            aria-label={t('nav.search.headerSearchHint')}
          >
            <span className="scene-header-bar__search-row">
              <span className="scene-header-bar__search-icon" aria-hidden="true">
                <Search size={12} />
              </span>
              <span className="scene-header-bar__search-label">{t('nav.search.headerSearchHint')}</span>
            </span>
          </button>
        </Tooltip>
        <GlobalSearchDialog open={searchOpen} onClose={() => setSearchOpen(false)} />
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
