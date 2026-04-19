import { useCallback, useEffect, useRef, useState } from 'react';
import { useToolCardHeightContract, type ToolCardCollapseReason } from './useToolCardHeightContract';

const TERMINAL_COLLAPSED_STATUSES = new Set(['completed', 'cancelled', 'error', 'rejected']);

interface ExpandedStateCache {
  expanded: boolean;
  isManual: boolean;
}

interface UseTerminalCardExpansionOptions {
  toolId: string | undefined;
  toolName: string;
  status: string;
  terminalSessionId?: string;
  onExpand?: () => void;
}

const expandedStateCache = new Map<string, ExpandedStateCache>();

function getCachedExpandedState(toolId: string | undefined): ExpandedStateCache | undefined {
  if (!toolId) {
    return undefined;
  }

  return expandedStateCache.get(toolId);
}

function setCachedExpandedState(toolId: string | undefined, expanded: boolean, isManual: boolean): void {
  if (!toolId) {
    return;
  }

  expandedStateCache.set(toolId, { expanded, isManual });
}

function isCollapsedTerminalStatus(status: string): boolean {
  return TERMINAL_COLLAPSED_STATUSES.has(status);
}

function getInitialExpandedState(toolId: string | undefined, status: string): boolean {
  const cached = getCachedExpandedState(toolId);
  if (cached) {
    return cached.expanded;
  }

  return !(isCollapsedTerminalStatus(status) || status === 'pending_confirmation');
}

export function useTerminalCardExpansion({
  toolId,
  toolName,
  status,
  terminalSessionId,
  onExpand,
}: UseTerminalCardExpansionOptions) {
  const [isExpanded, setIsExpanded] = useState(() => getInitialExpandedState(toolId, status));
  const hasInitializedExpand = useRef(false);
  const previousStatusRef = useRef(status);
  const {
    cardRootRef,
    applyExpandedState: applyHeightContractExpandedState,
  } = useToolCardHeightContract({
    toolId,
    toolName,
  });

  const setExpanded = useCallback((
    nextExpanded: boolean,
    options?: { isManual?: boolean; reason?: ToolCardCollapseReason },
  ) => {
    const isManual = options?.isManual ?? false;
    const reason = options?.reason ?? (isManual ? 'manual' : 'auto');

    if (nextExpanded !== isExpanded) {
      applyHeightContractExpandedState(isExpanded, nextExpanded, (nextValue) => {
        setIsExpanded(nextValue);
        setCachedExpandedState(toolId, nextValue, isManual);
      }, {
        reason,
        onExpand,
      });
    } else if (isManual) {
      setCachedExpandedState(toolId, nextExpanded, true);
    }
  }, [applyHeightContractExpandedState, isExpanded, onExpand, toolId]);

  const toggleExpanded = useCallback(() => {
    setExpanded(!isExpanded, { isManual: true, reason: 'manual' });
  }, [isExpanded, setExpanded]);

  useEffect(() => {
    if (!terminalSessionId || hasInitializedExpand.current) {
      return;
    }

    if (isCollapsedTerminalStatus(status)) {
      hasInitializedExpand.current = true;
      return;
    }

    const cached = getCachedExpandedState(toolId);
    if (cached === undefined || !cached.isManual) {
      setExpanded(true, { isManual: false, reason: 'auto' });
    }

    hasInitializedExpand.current = true;
  }, [setExpanded, status, terminalSessionId, toolId]);

  useEffect(() => {
    const prevStatus = previousStatusRef.current;
    previousStatusRef.current = status;

    const cached = getCachedExpandedState(toolId);
    if (cached?.isManual) {
      return;
    }

    if (status === 'running' && prevStatus !== 'running') {
      setExpanded(true, { isManual: false, reason: 'auto' });
    }

    if (!isCollapsedTerminalStatus(prevStatus) && isCollapsedTerminalStatus(status) && isExpanded) {
      setExpanded(false, { isManual: false, reason: 'auto' });
    }
  }, [isExpanded, setExpanded, status, toolId]);

  return {
    cardRootRef,
    isExpanded,
    setExpanded,
    toggleExpanded,
  };
}
