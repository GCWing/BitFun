import { useState, useCallback, useRef } from 'react';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';

export interface UseSubagentTimeoutControlResult {
  /** Whether timeout is currently disabled by user. */
  isTimeoutDisabled: boolean;
  /** Whether a toggle operation is in-flight. */
  isToggling: boolean;
  /** Whether the popover for extend options is open. */
  isPopoverOpen: boolean;
  /** Toggle timeout disable/enable. Returns true if action was taken, false if popover needed. */
  toggleTimeout: () => void;
  /** Extend timeout by specified seconds. */
  extendTimeout: (seconds: number) => void;
  /** Close the extend popover. */
  closePopover: () => void;
  /** Remaining seconds at the moment timeout was disabled (for popover display). */
  remainingAtDisable: number;
}

/**
 * Hook for controlling subagent timeout disable/restore/extend.
 *
 * @param subagentSessionId - The subagent session ID (needed for API call).
 * @param isRunning - Whether the subagent is currently running.
 * @param timeoutMs - Original timeout in ms.
 * @param remainingMs - Current remaining time in ms (null if no timeout or disabled).
 */
export function useSubagentTimeoutControl(
  subagentSessionId: string | undefined,
  isRunning: boolean,
  timeoutMs: number | undefined,
  remainingMs: number | null,
): UseSubagentTimeoutControlResult {
  // timeoutMs is part of the API surface but not directly used here;
  // remainingMs (derived from timeoutMs + elapsed time) drives the UI logic.
  void timeoutMs;

  const [isTimeoutDisabled, setIsTimeoutDisabled] = useState(false);
  const [isToggling, setIsToggling] = useState(false);
  const [isPopoverOpen, setIsPopoverOpen] = useState(false);
  const [remainingAtDisable, setRemainingAtDisable] = useState(0);
  const isRunningRef = useRef(isRunning);
  isRunningRef.current = isRunning;

  const closePopover = useCallback(() => {
    setIsPopoverOpen(false);
  }, []);

  const callApi = useCallback(async (
    action: { type: 'disable' } | { type: 'restore' } | { type: 'extend'; seconds: number },
  ) => {
    if (!subagentSessionId || !isRunningRef.current) return;
    setIsToggling(true);
    try {
      await agentAPI.setSubagentTimeout(subagentSessionId, action);
    } catch (_error) {
      // Rollback on failure.
      if (action.type === 'disable') {
        setIsTimeoutDisabled(false);
      } else {
        setIsTimeoutDisabled(true);
      }
    } finally {
      setIsToggling(false);
    }
  }, [subagentSessionId]);

  const toggleTimeout = useCallback(() => {
    if (!subagentSessionId || !isRunning) return;

    if (isTimeoutDisabled) {
      // Currently disabled -> want to restore.
      // Check remaining time to decide if popover is needed.
      const remaining = remainingMs ?? 0;
      const remainingSec = Math.ceil(remaining / 1000);
      if (remainingSec <= 30) {
        // Need popover: remaining too short.
        setRemainingAtDisable(remainingSec);
        setIsPopoverOpen(true);
        return;
      }
      // Direct restore.
      setIsTimeoutDisabled(false);
      callApi({ type: 'restore' });
    } else {
      // Currently enabled -> disable.
      setIsTimeoutDisabled(true);
      callApi({ type: 'disable' });
    }
  }, [isTimeoutDisabled, remainingMs, subagentSessionId, isRunning, callApi]);

  const extendTimeout = useCallback((seconds: number) => {
    if (!subagentSessionId || !isRunning) return;
    setIsTimeoutDisabled(false);
    setIsPopoverOpen(false);
    callApi({ type: 'extend', seconds });
  }, [subagentSessionId, isRunning, callApi]);

  return {
    isTimeoutDisabled,
    isToggling,
    isPopoverOpen,
    toggleTimeout,
    extendTimeout,
    closePopover,
    remainingAtDisable,
  };
}
