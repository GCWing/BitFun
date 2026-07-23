import { useCallback, useRef } from 'react';
export type ToolCardCollapseReason = 'manual' | 'auto';

interface UseToolCardHeightContractOptions {
  toolId: string | null | undefined;
  toolName: string;
  getCardHeight?: () => number | null;
}

interface ApplyHeightContractOptions {
  reason?: ToolCardCollapseReason;
  onExpand?: () => void;
  detail?: Record<string, unknown>;
}

export function useToolCardHeightContract({
  toolId,
  toolName,
  getCardHeight,
}: UseToolCardHeightContractOptions) {
  const cardRootRef = useRef<HTMLDivElement>(null);

  const dispatchToolCardToggle = useCallback(() => {
    window.dispatchEvent(new CustomEvent('tool-card-toggle'));
  }, []);

  const dispatchCollapseIntent = useCallback((
    reason: ToolCardCollapseReason,
    detail?: Record<string, unknown>,
  ) => {
    const cardHeight = getCardHeight?.()
      ?? cardRootRef.current?.getBoundingClientRect().height
      ?? null;

    window.dispatchEvent(new CustomEvent('flowchat:tool-card-collapse-intent', {
      detail: {
        toolId: toolId ?? null,
        toolName,
        cardHeight,
        reason,
        ...detail,
      },
    }));
  }, [getCardHeight, toolId, toolName]);

  const applyExpandedState = useCallback((
    currentExpanded: boolean,
    nextExpanded: boolean,
    setExpanded: (nextExpanded: boolean) => void,
    options?: ApplyHeightContractOptions,
  ) => {
    if (!nextExpanded && currentExpanded) {
      dispatchCollapseIntent(options?.reason ?? 'manual', options?.detail);
    }

    if (nextExpanded !== currentExpanded) {
      setExpanded(nextExpanded);
      dispatchToolCardToggle();
    }

    if (nextExpanded) {
      options?.onExpand?.();
    }
  }, [dispatchCollapseIntent, dispatchToolCardToggle]);

  return {
    cardRootRef,
    dispatchToolCardToggle,
    dispatchCollapseIntent,
    applyExpandedState,
  };
}
