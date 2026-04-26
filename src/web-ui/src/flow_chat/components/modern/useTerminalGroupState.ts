/**
 * Terminal-group expansion state for Modern FlowChat.
 */

import { useCallback, useState } from 'react';

interface UseTerminalGroupStateResult {
  /**
   * Expanded/collapsed state for each terminal group.
   * key: groupId, value: true means expanded.
   */
  terminalGroupStates: Map<string, boolean>;
  onTerminalGroupToggle: (groupId: string) => void;
  onExpandTerminalGroup: (groupId: string) => void;
  onCollapseTerminalGroup: (groupId: string) => void;
}

export function useTerminalGroupState(): UseTerminalGroupStateResult {
  const [terminalGroupStates, setTerminalGroupStates] = useState<Map<string, boolean>>(new Map());

  const onTerminalGroupToggle = useCallback((groupId: string) => {
    setTerminalGroupStates(prev => {
      const next = new Map(prev);
      const currentExpanded = prev.get(groupId) ?? false;
      next.set(groupId, !currentExpanded);
      return next;
    });
  }, []);

  const onExpandTerminalGroup = useCallback((groupId: string) => {
    setTerminalGroupStates(prev => {
      if (prev.get(groupId) === true) {
        return prev;
      }
      const next = new Map(prev);
      next.set(groupId, true);
      return next;
    });
  }, []);

  const onCollapseTerminalGroup = useCallback((groupId: string) => {
    setTerminalGroupStates(prev => {
      const next = new Map(prev);
      next.set(groupId, false);
      return next;
    });
  }, []);

  return {
    terminalGroupStates,
    onTerminalGroupToggle,
    onExpandTerminalGroup,
    onCollapseTerminalGroup,
  };
}
