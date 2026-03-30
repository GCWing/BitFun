/**
 * Terminal hook for state and event handling.
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { getTerminalService, TerminalService } from '../services';
import { createLogger } from '@/shared/utils/logger';
import type {
  SessionResponse,
  TerminalEvent,
  TerminalEventCallback,
} from '../types';

const log = createLogger('useTerminal');

export interface UseTerminalOptions {
  sessionId: string;
  autoConnect?: boolean;
  onEvent?: TerminalEventCallback;
  onOutput?: (data: string) => void;
  onReady?: () => void;
  onExit?: (exitCode?: number) => void;
  onError?: (message: string) => void;
  /** Called with the PTY dimensions stored alongside history, before onOutput. */
  onHistoryDims?: (cols: number, rows: number) => void;
}

export interface UseTerminalReturn {
  service: TerminalService;
  session: SessionResponse | null;
  isConnected: boolean;
  isLoading: boolean;
  error: string | null;
  write: (data: string) => Promise<void>;
  resize: (cols: number, rows: number) => Promise<void>;
  sendCtrlC: () => Promise<void>;
  close: () => Promise<void>;
  refresh: () => Promise<void>;
}

export function useTerminal(options: UseTerminalOptions): UseTerminalReturn {
  const {
    sessionId,
    autoConnect = true,
    onEvent,
    onOutput,
    onReady,
    onExit,
    onError,
    onHistoryDims,
  } = options;

  const [session, setSession] = useState<SessionResponse | null>(null);
  const [isConnected, setIsConnected] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const serviceRef = useRef<TerminalService>(getTerminalService());
  const unsubscribeRef = useRef<(() => void) | null>(null);

  // Use refs for callbacks to avoid re-creating handleEvent and re-triggering useEffect
  const onEventRef = useRef(onEvent);
  const onOutputRef = useRef(onOutput);
  const onReadyRef = useRef(onReady);
  const onExitRef = useRef(onExit);
  const onErrorRef = useRef(onError);
  const onHistoryDimsRef = useRef(onHistoryDims);

  // Keep refs updated
  useEffect(() => {
    onEventRef.current = onEvent;
    onOutputRef.current = onOutput;
    onReadyRef.current = onReady;
    onExitRef.current = onExit;
    onErrorRef.current = onError;
    onHistoryDimsRef.current = onHistoryDims;
  });

  // Stable event handler that uses refs
  const handleEvent = useCallback((event: TerminalEvent) => {
    if (event.sessionId !== sessionId) return;

    onEventRef.current?.(event);

    switch (event.type) {
      case 'output':
        onOutputRef.current?.((event as any).data);
        break;
      case 'ready':
        onReadyRef.current?.();
        break;
      case 'exit':
        onExitRef.current?.((event as any).exitCode);
        break;
      case 'error':
        onErrorRef.current?.((event as any).message);
        setError((event as any).message);
        break;
      case 'resize':
        // Backend resize confirmation requires no extra UI work.
        break;
    }
  }, [sessionId]); // Only depend on sessionId, not the callbacks

  useEffect(() => {
    const service = serviceRef.current;
    let cancelled = false;

    const connect = async () => {
      try {
        setIsLoading(true);
        setError(null);

        if (autoConnect && !service.isConnected()) {
          await service.connect();
        }

        if (cancelled) return;

        setIsConnected(service.isConnected());

        // Get session info
        const sessionInfo = await service.getSession(sessionId);
        if (cancelled) return;

        setSession(sessionInfo);

        // Replay output history BEFORE subscribing to live events.
        // This prevents overlap: history covers [past → now], events cover [now → future].
        // If we subscribed first, events arriving between subscribe and getHistory would
        // appear in both the event stream and the history buffer, causing duplicate output.
        try {
          const historyResponse = await service.getHistory(sessionId);
          if (!cancelled && historyResponse.data) {
            // Notify before queuing data so the terminal can resize to the correct
            // dimensions before history is written to the buffer.
            onHistoryDimsRef.current?.(historyResponse.cols, historyResponse.rows);
            onOutputRef.current?.(historyResponse.data);
          }
        } catch (histErr) {
          // History replay is optional — a failed fetch must not break the terminal.
          log.warn('Failed to fetch terminal history', { sessionId, error: histErr });
        }

        if (cancelled) return;

        // Subscribe to live events AFTER history replay to eliminate overlap.
        const unsubscribe = service.onSessionEvent(sessionId, handleEvent);
        if (cancelled) {
          unsubscribe();
          return;
        }
        unsubscribeRef.current = unsubscribe;

        setIsLoading(false);
      } catch (err) {
        if (cancelled) return;
        const message = err instanceof Error ? err.message : 'Connection failed';
        setError(message);
        setIsLoading(false);
        log.error('Failed to connect', { sessionId, error: err });
      }
    };

    connect();

    return () => {
      cancelled = true;
      // Clean up the subscription
      if (unsubscribeRef.current) {
        unsubscribeRef.current();
        unsubscribeRef.current = null;
      }
    };
  }, [sessionId, autoConnect, handleEvent]);

  const write = useCallback(async (data: string) => {
    try {
      await serviceRef.current.write(sessionId, data);
    } catch (err) {
      log.error('Failed to write', { sessionId, error: err });
      throw err;
    }
  }, [sessionId]);

  const resize = useCallback(async (cols: number, rows: number) => {
    try {
      await serviceRef.current.resize(sessionId, cols, rows);
    } catch (err) {
      log.error('Failed to resize', { sessionId, cols, rows, error: err });
      throw err;
    }
  }, [sessionId]);

  const sendCtrlC = useCallback(async () => {
    try {
      await serviceRef.current.sendCtrlC(sessionId);
    } catch (err) {
      log.error('Failed to send Ctrl+C', { sessionId, error: err });
      throw err;
    }
  }, [sessionId]);

  const close = useCallback(async () => {
    try {
      await serviceRef.current.closeSession(sessionId);
      setSession(null);
    } catch (err) {
      log.error('Failed to close session', { sessionId, error: err });
      throw err;
    }
  }, [sessionId]);

  const refresh = useCallback(async () => {
    try {
      const sessionInfo = await serviceRef.current.getSession(sessionId);
      setSession(sessionInfo);
    } catch (err) {
      log.error('Failed to refresh session', { sessionId, error: err });
      throw err;
    }
  }, [sessionId]);

  return {
    service: serviceRef.current,
    session,
    isConnected,
    isLoading,
    error,
    write,
    resize,
    sendCtrlC,
    close,
    refresh,
  };
}
