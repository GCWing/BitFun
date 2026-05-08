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

import React, { useCallback, useMemo, useRef } from 'react';
import { ArrowLeft, ArrowRight } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useNavSceneStore } from '../../stores/navSceneStore';
import { useI18n } from '../../../infrastructure/i18n';
import { PanelLeftIcon } from '../TitleBar/PanelIcons';
import { createLogger } from '@/shared/utils/logger';
import { isMacOSDesktopRuntime } from '@/infrastructure/runtime';
import './NavBar.scss';
import { workspaceAPI } from '@/infrastructure';

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
    return isMacOSDesktopRuntime();
  }, []);
  const showSceneNav = useNavSceneStore(s => s.showSceneNav);
  const navSceneId = useNavSceneStore(s => s.navSceneId);
  const goBack = useNavSceneStore(s => s.goBack);
  const goForward = useNavSceneStore(s => s.goForward);
  const canGoBack = showSceneNav && !!navSceneId;
  const canGoForward = !showSceneNav && !!navSceneId;
  const isDraggingRef = useRef(false);

  const handleBarMouseDown = (() => {
    isDraggingRef.current = true;
  })

  const handleBarMouseMove = async () => {
    if (isDraggingRef.current) {
      try {
        await workspaceAPI.window_start_dragging();
      } catch (error) {
        log.debug('startDragging failed', error);
      }
    }

  };

  const handlebarMouseUp = (() => {
    isDraggingRef.current = false;
  });

  const handleBarDoubleClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement | null;
    if (!target) return;
    if (target.closest(INTERACTIVE_SELECTOR)) return;
    onMaximize?.();
  }, [onMaximize]);

  const rootClassName = `bitfun-nav-bar${isCollapsed ? ' bitfun-nav-bar--collapsed' : ''}${isMacOS ? ' bitfun-nav-bar--macos' : ''} ${className}`;

  if (isCollapsed) {
    return (
      <div className={rootClassName} role="toolbar" aria-label={t('nav.aria.navControl')} onMouseDown={handleBarMouseDown} onMouseMove={handleBarMouseMove} onMouseUp={handlebarMouseUp} onDoubleClick={handleBarDoubleClick}>
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
    <div className={rootClassName} role="toolbar" aria-label={t('nav.aria.navControl')} onMouseDown={handleBarMouseDown} onMouseMove={handleBarMouseMove} onMouseUp={handlebarMouseUp} onDoubleClick={handleBarDoubleClick}>
      <Tooltip content={t('header.collapseLeftPanel')} placement="bottom" followCursor>
        <button
          type="button"
          className="bitfun-nav-bar__panel-toggle"
          onClick={onExpandNav}
          aria-label={t('header.collapseLeftPanel')}
        >
          <PanelLeftIcon size={13} />
        </button>
      </Tooltip>

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
