import React, { useCallback, useRef } from 'react';
import { Toolbar } from '@openbitfun/ui';
import { WindowControls } from '@/app/components/WindowControls';
import { supportsNativeWindowDragging } from '@/infrastructure/runtime';
import { createLogger } from '@/shared/utils/logger';
import { useSceneStore } from '../../stores/sceneStore';
import SceneBar from '../SceneBar/SceneBar';
import { SceneChromeHost } from './SceneChrome';
import './SceneTopBar.scss';

const log = createLogger('SceneTopBar');

const INTERACTIVE_SELECTOR =
  'button, input, textarea, select, a, [role="button"], [contenteditable="true"], .window-controls';

function blocksWindowChromeInteraction(target: HTMLElement): boolean {
  const interactive = target.closest<HTMLElement>(INTERACTIVE_SELECTOR);
  return interactive !== null && interactive.getAttribute('role') !== 'tab';
}

interface SceneTopBarProps {
  className?: string;
  onMinimize?: () => void;
  onMaximize?: () => void;
  onClose?: () => void;
  isMaximized?: boolean;
}

const SceneTopBar: React.FC<SceneTopBarProps> = ({
  className = '',
  onMinimize,
  onMaximize,
  onClose,
  isMaximized = false,
}) => {
  const openTabCount = useSceneStore(state => state.openTabs.length);
  const hasTabs = openTabCount > 0;
  const isSingleTab = openTabCount <= 1;
  const canDragWindow = supportsNativeWindowDragging();
  const lastMouseDownTimeRef = useRef(0);
  const hasWindowControls = Boolean(onMinimize && onMaximize && onClose);

  const handleMouseDown = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    if (!canDragWindow || !isSingleTab || event.button !== 0) return;
    const target = event.target as HTMLElement | null;
    if (!target || blocksWindowChromeInteraction(target)) return;

    const now = Date.now();
    const timeSinceLastMouseDown = now - lastMouseDownTimeRef.current;
    lastMouseDownTimeRef.current = now;
    if (timeSinceLastMouseDown < 500 && timeSinceLastMouseDown > 50) return;

    void (async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        await getCurrentWindow().startDragging();
      } catch (error) {
        log.debug('startDragging failed', error);
      }
    })();
  }, [canDragWindow, isSingleTab]);

  const handleDoubleClick = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    if (!isSingleTab) return;
    const target = event.target as HTMLElement | null;
    if (!target || blocksWindowChromeInteraction(target)) return;
    onMaximize?.();
  }, [isSingleTab, onMaximize]);

  return (
    <Toolbar
      bordered={hasTabs}
      className={`openbitfun-scene-top-bar ${className}`.trim()}
      onMouseDown={handleMouseDown}
      onDoubleClick={handleDoubleClick}
      data-openbitfun-scene="workbench"
      data-openbitfun-part="topBar"
      leading={<SceneBar />}
      size="md"
      trailing={<>
      <SceneChromeHost
        className="openbitfun-scene-top-bar__actions"
        data-openbitfun-scene="workbench"
        data-openbitfun-part="sceneActions"
      />
      {hasWindowControls ? (
        <div
          className="openbitfun-scene-top-bar__window-controls"
          data-openbitfun-component="scene-bar"
          data-openbitfun-part="controls"
        >
          <WindowControls
            onMinimize={onMinimize!}
            onToggleMaximize={onMaximize!}
            onClose={onClose!}
            maximized={isMaximized}
          />
        </div>
      ) : null}
      </>}
    />
  );
};

export default SceneTopBar;
