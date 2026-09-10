import React, { useCallback } from 'react';
import {
  CanvasStoreModeContext,
  ContentCanvas,
} from '../../components/panels/content-canvas';
import { useBottomTerminalCanvasStore } from '../../components/panels/content-canvas/stores';
import { TAB_EVENTS } from '../../components/panels/content-canvas/types';
import { usePanelTabCoordinator } from '../../components/panels/content-canvas/hooks/usePanelTabCoordinator';
import './BottomTerminalPane.scss';

interface BottomTerminalPaneProps {
  workspacePath?: string;
  isSceneActive?: boolean;
  onExpand: () => void;
  onCollapse: () => void;
  terminalResizeSuspended?: boolean;
}

const BottomTerminalPane: React.FC<BottomTerminalPaneProps> = ({
  workspacePath,
  isSceneActive = true,
  onExpand,
  onCollapse,
  terminalResizeSuspended = false,
}) => {
  const visibleTabCount = useBottomTerminalCanvasStore(state => (
    [state.primaryGroup, state.secondaryGroup, state.tertiaryGroup]
      .reduce((count, group) => count + group.tabs.filter(tab => !tab.isHidden).length, 0)
  ));
  const { expandPanel, collapsePanel } = usePanelTabCoordinator({
    visibleTabCount,
    expandEventName: TAB_EVENTS.EXPAND_BOTTOM_TERMINAL_PANEL,
    onExpand,
    onCollapse,
  });
  const handleInteraction = useCallback(async (_itemId: string, _userInput: string) => {
    // Terminal tabs do not use ContentCanvas interaction callbacks.
  }, []);

  const handleBeforeClose = useCallback(async () => true, []);

  return (
    <CanvasStoreModeContext.Provider value="bottom-terminal">
      <div data-openbitfun-component="bottom-terminal-pane" data-openbitfun-part="root" className="openbitfun-bottom-terminal-pane">
        <ContentCanvas
          workspacePath={workspacePath}
          mode="bottom-terminal"
          isSceneActive={isSceneActive}
          onInteraction={handleInteraction}
          onBeforeClose={handleBeforeClose}
          disablePopOut
          createTabEventName={TAB_EVENTS.BOTTOM_TERMINAL_CREATE_TAB}
          onReveal={expandPanel}
          onCollapsePanel={collapsePanel}
          terminalResizeSuspended={terminalResizeSuspended}
        />
      </div>
    </CanvasStoreModeContext.Provider>
  );
};

export default BottomTerminalPane;
