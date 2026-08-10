import { useCallback, useLayoutEffect, useRef } from 'react';
export type ToolCardCollapseReason = 'manual' | 'auto';

interface UseToolCardHeightContractOptions {
  toolId: string | null | undefined;
  toolName: string;
  getAnchorElement?: () => HTMLElement | null;
}

interface ApplyHeightContractOptions {
  reason?: ToolCardCollapseReason;
  onExpand?: () => void;
  detail?: Record<string, unknown>;
}

export function useToolCardHeightContract({
  toolId,
  toolName,
  getAnchorElement,
}: UseToolCardHeightContractOptions) {
  const cardRootRef = useRef<HTMLDivElement>(null);
  const lastMeasuredHeightRef = useRef(0);
  const previousMeasuredHeightRef = useRef(0);

  useLayoutEffect(() => {
    const nextHeight = cardRootRef.current?.getBoundingClientRect().height ?? 0;
    if (nextHeight <= 0) {
      return;
    }
    previousMeasuredHeightRef.current = lastMeasuredHeightRef.current;
    lastMeasuredHeightRef.current = nextHeight;
  });

  const dispatchToolCardToggle = useCallback(() => {
    window.dispatchEvent(new CustomEvent('tool-card-toggle'));
  }, []);

  const dispatchCollapseIntent = useCallback((
    reason: ToolCardCollapseReason,
    detail?: Record<string, unknown>,
  ) => {
    const measuredHeight = cardRootRef.current?.getBoundingClientRect().height ?? 0;
    const cardHeight = Math.max(
      measuredHeight,
      lastMeasuredHeightRef.current,
      previousMeasuredHeightRef.current,
    ) || null;

    window.dispatchEvent(new CustomEvent('flowchat:tool-card-collapse-intent', {
      detail: {
        ...detail,
        toolId: toolId ?? null,
        toolName,
        cardHeight,
        anchorElement: getAnchorElement?.() ?? cardRootRef.current,
        reason,
      },
    }));
  }, [getAnchorElement, toolId, toolName]);

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
