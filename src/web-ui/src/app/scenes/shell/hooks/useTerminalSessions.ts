import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getTerminalService } from '@/tools/terminal';
import type { TerminalService } from '@/tools/terminal';
import type { SessionResponse, TerminalEvent } from '@/tools/terminal/types/session';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import type { TerminalConfig } from '@/infrastructure/config/types';
import { createLogger } from '@/shared/utils/logger';
import {
  isSessionRunning,
  MANUAL_SOURCE,
  type ShellEntry,
} from './shellEntryTypes';

const log = createLogger('useTerminalSessions');

interface UseTerminalSessionsOptions {
  workspacePath?: string;
  isRemote: boolean;
  currentConnectionId: string | null;
}

interface UseTerminalSessionsReturn {
  sessions: SessionResponse[];
  sessionMap: Map<string, SessionResponse>;
  refreshSessions: () => Promise<void>;
  startEntrySession: (entry: ShellEntry) => Promise<boolean>;
  createManualSession: (shellType?: string) => Promise<SessionResponse | null>;
  stopEntrySession: (entry: ShellEntry) => Promise<void>;
  closeSessionIfPresent: (sessionId: string) => Promise<void>;
  renameSessionLocally: (sessionId: string, newName: string) => void;
  hasSession: (sessionId: string) => boolean;
}

async function getDefaultShellType(): Promise<string | undefined> {
  try {
    const config = await configManager.getConfig<TerminalConfig>('terminal');
    return config?.default_shell || undefined;
  } catch {
    return undefined;
  }
}

function dispatchTerminalDestroyed(sessionId: string) {
  if (typeof window === 'undefined') {
    return;
  }

  window.dispatchEvent(new CustomEvent('terminal-session-destroyed', { detail: { sessionId } }));
}

function dispatchTerminalRenamed(sessionId: string, newName: string) {
  if (typeof window === 'undefined') {
    return;
  }

  window.dispatchEvent(new CustomEvent('terminal-session-renamed', { detail: { sessionId, newName } }));
}

export function useTerminalSessions(
  options: UseTerminalSessionsOptions,
): UseTerminalSessionsReturn {
  const { workspacePath, isRemote, currentConnectionId } = options;
  const [sessions, setSessions] = useState<SessionResponse[]>([]);
  const serviceRef = useRef<TerminalService | null>(null);

  const sessionMap = useMemo(
    () => new Map(sessions.map((session) => [session.id, session])),
    [sessions],
  );

  const refreshSessions = useCallback(async () => {
    const service = serviceRef.current;
    if (!service) {
      return;
    }

    try {
      const allSessions = await service.listSessions();
      const filtered = allSessions.filter((session) => {
        const isRemoteSession = session.shellType === 'Remote';
        if (isRemote) {
          return isRemoteSession && session.connectionId === currentConnectionId;
        }
        return !isRemoteSession;
      });
      setSessions(filtered);
    } catch (error) {
      log.error('Failed to list sessions', error);
    }
  }, [currentConnectionId, isRemote]);

  useEffect(() => {
    const service = getTerminalService();
    serviceRef.current = service;

    const init = async () => {
      try {
        await service.connect();
        await refreshSessions();
      } catch (error) {
        log.error('Failed to connect terminal service', error);
      }
    };

    void init();

    const unsubscribe = service.onEvent((event: TerminalEvent) => {
      if (event.type === 'ready' || event.type === 'exit') {
        void refreshSessions();
      }
    });

    return () => unsubscribe();
  }, [refreshSessions]);

  const closeSessionIfPresent = useCallback(async (sessionId: string) => {
    const service = serviceRef.current;
    if (!service || !sessionMap.has(sessionId)) {
      return;
    }

    try {
      await service.closeSession(sessionId);
      dispatchTerminalDestroyed(sessionId);
    } catch (error) {
      log.error('Failed to close terminal session', { sessionId, error });
    }
  }, [sessionMap]);

  const startEntrySession = useCallback(async (entry: ShellEntry): Promise<boolean> => {
    const service = serviceRef.current;
    const existingSession = sessionMap.get(entry.sessionId);
    if (!service) {
      return false;
    }

    try {
      if (existingSession && !isSessionRunning(existingSession)) {
        await service.closeSession(entry.sessionId);
      }

      const shellType = entry.shellType ?? await getDefaultShellType();
      await service.createSession({
        sessionId: entry.sessionId,
        workingDirectory: entry.workingDirectory ?? entry.cwd ?? workspacePath,
        name: entry.name,
        shellType,
        source: entry.source,
      });

      if (entry.startupCommand?.trim()) {
        await new Promise((resolve) => setTimeout(resolve, 800));
        try {
          await service.sendCommand(entry.sessionId, entry.startupCommand);
        } catch (error) {
          log.error('Failed to run startup command', { sessionId: entry.sessionId, error });
        }
      }

      await refreshSessions();
      return true;
    } catch (error) {
      log.error('Failed to start terminal entry', { entry, error });
      return false;
    }
  }, [refreshSessions, sessionMap, workspacePath]);

  const createManualSession = useCallback(async (shellTypeOverride?: string): Promise<SessionResponse | null> => {
    const service = serviceRef.current;
    if (!service) {
      return null;
    }

    try {
      const shellType = shellTypeOverride ?? await getDefaultShellType();
      const nextIndex = sessions.filter((session) => session.source === MANUAL_SOURCE).length + 1;
      const session = await service.createSession({
        workingDirectory: workspacePath,
        name: `Shell ${nextIndex}`,
        shellType,
        source: MANUAL_SOURCE,
      });

      await refreshSessions();
      return session;
    } catch (error) {
      log.error('Failed to create manual terminal', error);
      return null;
    }
  }, [refreshSessions, sessions, workspacePath]);

  const stopEntrySession = useCallback(async (entry: ShellEntry) => {
    const session = sessionMap.get(entry.sessionId);
    if (!session || !isSessionRunning(session)) {
      return;
    }

    await closeSessionIfPresent(entry.sessionId);
    await refreshSessions();
  }, [closeSessionIfPresent, refreshSessions, sessionMap]);

  const renameSessionLocally = useCallback((sessionId: string, newName: string) => {
    if (!sessionMap.has(sessionId)) {
      return;
    }

    setSessions((prev) =>
      prev.map((session) => (session.id === sessionId ? { ...session, name: newName } : session)),
    );
    dispatchTerminalRenamed(sessionId, newName);
  }, [sessionMap]);

  const hasSession = useCallback((sessionId: string) => sessionMap.has(sessionId), [sessionMap]);

  return {
    sessions,
    sessionMap,
    refreshSessions,
    startEntrySession,
    createManualSession,
    stopEntrySession,
    closeSessionIfPresent,
    renameSessionLocally,
    hasSession,
  };
}
