import React, { useCallback } from 'react';
import {
  CanvasStoreModeContext,
  ContentCanvas,
} from '../../components/panels/content-canvas';
import { TAB_EVENTS } from '../../components/panels/content-canvas/types';
import './BottomTerminalPane.scss';

interface BottomTerminalPaneProps {
  workspacePath?: string;
  isSceneActive?: boolean;
  isCollapsed: boolean;
  onExpand: () => void;
  onCollapse: () => void;
  terminalResizeSuspended?: boolean;
}

const BottomTerminalPane: React.FC<BottomTerminalPaneProps> = ({
  workspacePath,
  isSceneActive = true,
  isCollapsed,
  onExpand,
  onCollapse,
  terminalResizeSuspended = false,
}) => {
  const handleInteraction = useCallback(async (_itemId: string, _userInput: string) => {
    // Terminal tabs do not use ContentCanvas interaction callbacks.
  }, []);

  const handleBeforeClose = useCallback(async () => true, []);

  return (
    <CanvasStoreModeContext.Provider value="bottom-terminal">
      <div className="bitfun-bottom-terminal-pane">
        <ContentCanvas
          workspacePath={workspacePath}
          mode="bottom-terminal"
          isSceneActive={isSceneActive}
          onInteraction={handleInteraction}
          onBeforeClose={handleBeforeClose}
          disablePopOut
          createTabEventName={TAB_EVENTS.BOTTOM_TERMINAL_CREATE_TAB}
          expandPanelEventName={TAB_EVENTS.EXPAND_BOTTOM_TERMINAL_PANEL}
          isPanelCollapsed={isCollapsed}
          onExpandPanel={onExpand}
          onCollapsePanel={onCollapse}
          terminalResizeSuspended={terminalResizeSuspended}
        />
      </div>
    </CanvasStoreModeContext.Provider>
  );
};

export default BottomTerminalPane;
