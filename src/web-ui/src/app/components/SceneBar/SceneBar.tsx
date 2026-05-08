/**
 * SceneBar — horizontal scene-level tab bar (32px).
 *
 * Delegates state to useSceneManager.
 * AI Agent tab shows the current session title as a subtitle.
 */

import React, { useCallback, useRef } from 'react';
import SceneTab from './SceneTab';
import { WindowControls } from '@/component-library';
import { useSceneManager } from '../../hooks/useSceneManager';
import { useCurrentSessionTitle } from '../../hooks/useCurrentSessionTitle';
import { useCurrentSettingsTabTitle } from '../../hooks/useCurrentSettingsTabTitle';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { createLogger } from '@/shared/utils/logger';
import './SceneBar.scss';
import { workspaceAPI } from '@/infrastructure';

const log = createLogger('SceneBar');

const INTERACTIVE_SELECTOR =
  'button, input, textarea, select, a, [role="button"], [contenteditable="true"], .window-controls';

interface SceneBarProps {
  className?: string;
  onMinimize?: () => void;
  onMaximize?: () => void;
  onClose?: () => void;
  isMaximized?: boolean;
}

const SceneBar: React.FC<SceneBarProps> = ({
  className = '',
  onMinimize,
  onMaximize,
  onClose,
  isMaximized = false,
}) => {
  const { openTabs, activeTabId, tabDefs, activateScene, closeScene } = useSceneManager();
  const sessionTitle = useCurrentSessionTitle();
  const settingsTabTitle = useCurrentSettingsTabTitle();
  const { t } = useI18n('common');
  const hasWindowControls = !!(onMinimize && onMaximize && onClose);
  const sceneBarClassName = `bitfun-scene-bar ${!hasWindowControls ? 'bitfun-scene-bar--no-controls' : ''} ${className}`.trim();
  const isSingleTab = openTabs.length <= 1;
  const tabCount = Math.max(openTabs.length, 1);
  const tabsStyle = {
    ['--scene-tab-count' as string]: tabCount,
  } as React.CSSProperties;
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
    if (!isSingleTab) return;
    const target = e.target as HTMLElement | null;
    if (!target) return;
    if (target.closest(INTERACTIVE_SELECTOR)) return;
    onMaximize?.();
  }, [isSingleTab, onMaximize]);

  return (
    <div
      className={sceneBarClassName}
      role="tablist"
      aria-label="Scene tabs"
      onMouseDown={handleBarMouseDown}
      onMouseMove={handleBarMouseMove}
      onMouseUp={handlebarMouseUp}
      onDoubleClick={handleBarDoubleClick}
    >
      <div className="bitfun-scene-bar__tabs" style={tabsStyle}>
        {openTabs.map(tab => {
          const def = tabDefs.find(d => d.id === tab.id);
          if (!def) return null;
          const translatedLabel = def.labelKey ? t(def.labelKey) : def.label;
          const subtitle =
            (tab.id === 'session' && sessionTitle ? sessionTitle : undefined)
            ?? (tab.id === 'settings' && settingsTabTitle ? settingsTabTitle : undefined);
          return (
            <SceneTab
              key={tab.id}
              tab={tab}
              def={{ ...def, label: translatedLabel }}
              isActive={tab.id === activeTabId}
              subtitle={subtitle}
              onActivate={activateScene}
              onClose={closeScene}
            />
          );
        })}
      </div>

      {hasWindowControls && (
        <div className="bitfun-scene-bar__controls">
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

export default SceneBar;
