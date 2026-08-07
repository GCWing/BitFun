import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';

export type FollowOutputEnterReason = 'jump-to-latest' | 'new-turn' | 'streaming-resumed';
export type FollowOutputExitReason =
  | 'session-changed'
  | 'user-scroll'
  | 'scroll-to-turn'
  | 'scroll-to-index';

interface UseFlowChatFollowOutputOptions {
  activeSessionId?: string;
  latestTurnId: string | null;
  virtualItemCount: number;
  isStreaming: boolean;
  isViewportActive: boolean;
  scrollerRef: RefObject<HTMLElement | null>;
  scrollToTail: (behavior: ScrollBehavior) => void;
}

interface UseFlowChatFollowOutputResult {
  isFollowingOutput: boolean;
  enterFollowOutput: (reason: FollowOutputEnterReason) => void;
  exitFollowOutput: (reason: FollowOutputExitReason) => void;
  scheduleFollowToLatest: () => void;
  handleUserScrollIntent: () => void;
  handleScroll: () => void;
}

const BOTTOM_EPSILON_PX = 2;

function naturalTailScrollTop(scroller: HTMLElement): number {
  return Math.max(0, scroller.scrollHeight - scroller.clientHeight);
}

export function useFlowChatFollowOutput({
  activeSessionId,
  latestTurnId,
  virtualItemCount,
  isStreaming,
  isViewportActive,
  scrollerRef,
  scrollToTail,
}: UseFlowChatFollowOutputOptions): UseFlowChatFollowOutputResult {
  const [isFollowingOutput, setIsFollowingOutput] = useState(false);
  const isFollowingOutputRef = useRef(false);
  const isStreamingRef = useRef(isStreaming);
  const isViewportActiveRef = useRef(isViewportActive);
  const followFrameRef = useRef<number | null>(null);
  const previousSessionIdRef = useRef(activeSessionId);
  const previousLatestTurnIdRef = useRef<string | null>(latestTurnId);
  const hasMountedRef = useRef(false);

  isFollowingOutputRef.current = isFollowingOutput;
  isStreamingRef.current = isStreaming;
  isViewportActiveRef.current = isViewportActive;

  const stopFollowFrame = useCallback(() => {
    if (followFrameRef.current !== null) {
      cancelAnimationFrame(followFrameRef.current);
      followFrameRef.current = null;
    }
  }, []);

  const runFollowFrame = useCallback(() => {
    followFrameRef.current = null;
    if (
      !isFollowingOutputRef.current ||
      !isStreamingRef.current ||
      !isViewportActiveRef.current ||
      document.hidden
    ) {
      return;
    }

    const scroller = scrollerRef.current;
    if (scroller) {
      const target = naturalTailScrollTop(scroller);
      if (Math.abs(target - scroller.scrollTop) > BOTTOM_EPSILON_PX) {
        scroller.scrollTop = target;
      }
    }
    followFrameRef.current = requestAnimationFrame(runFollowFrame);
  }, [scrollerRef]);

  const startFollowFrame = useCallback(() => {
    if (followFrameRef.current === null && isFollowingOutputRef.current && isStreamingRef.current) {
      followFrameRef.current = requestAnimationFrame(runFollowFrame);
    }
  }, [runFollowFrame]);

  const enterFollowOutput = useCallback((reason: FollowOutputEnterReason) => {
    if (!isViewportActiveRef.current) {
      return;
    }
    isFollowingOutputRef.current = true;
    setIsFollowingOutput(true);
    scrollToTail(reason === 'jump-to-latest' ? 'smooth' : 'auto');
    startFollowFrame();
  }, [scrollToTail, startFollowFrame]);

  const exitFollowOutput = useCallback((_reason: FollowOutputExitReason) => {
    isFollowingOutputRef.current = false;
    setIsFollowingOutput(false);
    stopFollowFrame();
  }, [stopFollowFrame]);

  const scheduleFollowToLatest = useCallback(() => {
    if (!isFollowingOutputRef.current || !isViewportActiveRef.current) {
      return;
    }
    scrollToTail('auto');
    startFollowFrame();
  }, [scrollToTail, startFollowFrame]);

  const handleUserScrollIntent = useCallback(() => {
    exitFollowOutput('user-scroll');
  }, [exitFollowOutput]);

  const handleScroll = useCallback(() => {
    // Scroll events describe the resulting viewport position, but do not prove user intent.
    // Layout growth and virtualizer remeasurement can emit them while output follow still owns
    // the viewport. Explicit wheel, touch, and keyboard handlers release that ownership instead.
  }, []);

  useEffect(() => {
    if (!hasMountedRef.current) {
      hasMountedRef.current = true;
      if (isStreaming && virtualItemCount > 0) {
        enterFollowOutput('streaming-resumed');
      }
      return;
    }

    if (previousSessionIdRef.current !== activeSessionId) {
      previousSessionIdRef.current = activeSessionId;
      previousLatestTurnIdRef.current = latestTurnId;
      exitFollowOutput('session-changed');
      if (isStreaming && virtualItemCount > 0) {
        enterFollowOutput('streaming-resumed');
      }
      return;
    }

    const isNewTurn = Boolean(latestTurnId && latestTurnId !== previousLatestTurnIdRef.current);
    previousLatestTurnIdRef.current = latestTurnId;
    if (isNewTurn && virtualItemCount > 0) {
      enterFollowOutput('new-turn');
    }
  }, [
    activeSessionId,
    enterFollowOutput,
    exitFollowOutput,
    isStreaming,
    latestTurnId,
    virtualItemCount,
  ]);

  useEffect(() => {
    if (!isViewportActive) {
      stopFollowFrame();
      return;
    }
    if (isFollowingOutput && isStreaming) {
      scheduleFollowToLatest();
    }
  }, [isFollowingOutput, isStreaming, isViewportActive, scheduleFollowToLatest, stopFollowFrame]);

  useEffect(() => {
    const handleVisibilityChange = () => {
      if (!document.hidden) {
        scheduleFollowToLatest();
      }
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => document.removeEventListener('visibilitychange', handleVisibilityChange);
  }, [scheduleFollowToLatest]);

  useEffect(() => stopFollowFrame, [stopFollowFrame]);

  return {
    isFollowingOutput,
    enterFollowOutput,
    exitFollowOutput,
    scheduleFollowToLatest,
    handleUserScrollIntent,
    handleScroll,
  };
}
