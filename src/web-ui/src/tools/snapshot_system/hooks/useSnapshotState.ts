import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { SnapshotStateManager, SessionState, SnapshotFile } from '../core/SnapshotStateManager';
import { SnapshotEventBus, SNAPSHOT_EVENTS } from '../core/SnapshotEventBus';
import { DiffDisplayEngine, CompactDiffResult, FullDiffResult } from '../core/DiffDisplayEngine';
import SnapshotLazyLoader from '../core/SnapshotLazyLoader';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useSnapshotState');

interface UseSnapshotStateReturn {
  sessionState: SessionState | null;
  files: SnapshotFile[];
  loading: boolean;
  error: string | null;

  refreshSession: () => Promise<void>;
  acceptFile: (filePath: string) => Promise<void>;
  rejectFile: (filePath: string) => Promise<void>;
  acceptSession: () => Promise<void>;
  rejectSession: () => Promise<void>;
  acceptBlock: (filePath: string, blockId: string) => Promise<void>;
  rejectBlock: (filePath: string, blockId: string) => Promise<void>;
  
  getCompactDiff: (filePath: string) => CompactDiffResult | null;
  getFullDiff: (filePath: string) => FullDiffResult | null;
  
  clearError: () => void;
}

export const useSnapshotState = (sessionId?: string): UseSnapshotStateReturn => {
  const { t } = useTranslation('flow-chat');
  const [sessionState, setSessionState] = useState<SessionState | null>(null);
  const [files, setFiles] = useState<SnapshotFile[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Track the active session to avoid applying stale events after session switches.
  const activeSessionIdRef = useRef<string | undefined>(sessionId);

  const stateManager = SnapshotStateManager.getInstance();
  const eventBus = SnapshotEventBus.getInstance();
  const diffEngine = useMemo(() => new DiffDisplayEngine(), []);

  const refreshSession = useCallback(async () => {
    if (!sessionId) return;

    setLoading(true);
    setError(null);
    
    try {
      await SnapshotLazyLoader.ensureInitialized();
      
      await stateManager.refreshSessionState(sessionId);
      const newSessionState = stateManager.getSessionState(sessionId);
      const newFiles = stateManager.getSessionFiles(sessionId);
      
      setSessionState(newSessionState);
      setFiles(newFiles);
    } catch (err) {
      log.error('Failed to refresh session state', { sessionId, error: err });
      setError(t('snapshotSystem.errors.refreshSessionFailed'));
    } finally {
      setLoading(false);
    }
  }, [sessionId, stateManager, t]);

  const acceptFile = useCallback(async (filePath: string) => {
    if (!sessionId) return;

    try {
      setError(null);
      
      await SnapshotLazyLoader.ensureInitialized();
      
      eventBus.emit(SNAPSHOT_EVENTS.USER_ACCEPT_FILE, { filePath }, sessionId, filePath);
      
      await stateManager.handleUserFileAction(sessionId, filePath, 'accept');
      
    } catch (err) {
      log.error('Failed to accept file', { sessionId, filePath, error: err });
      setError(t('snapshotSystem.errors.acceptFileFailed'));
      throw err;
    }
  }, [sessionId, eventBus, stateManager, t]);

  const rejectFile = useCallback(async (filePath: string) => {
    if (!sessionId) return;

    try {
      setError(null);
      
      eventBus.emit(SNAPSHOT_EVENTS.USER_REJECT_FILE, { filePath }, sessionId, filePath);
      
      await stateManager.handleUserFileAction(sessionId, filePath, 'reject');
      
    } catch (err) {
      log.error('Failed to reject file', { sessionId, filePath, error: err });
      setError(t('snapshotSystem.errors.rejectFileFailed'));
      throw err;
    }
  }, [sessionId, eventBus, stateManager, t]);

  const acceptSession = useCallback(async () => {
    if (!sessionId) return;

    try {
      setError(null);
      
      eventBus.emit(SNAPSHOT_EVENTS.USER_ACCEPT_SESSION, {}, sessionId);
      await stateManager.handleUserSessionAction(sessionId, 'accept');
      
    } catch (err) {
      log.error('Failed to accept session', { sessionId, error: err });
      setError(t('snapshotSystem.errors.acceptSessionFailed'));
      throw err;
    }
  }, [sessionId, eventBus, stateManager, t]);

  const rejectSession = useCallback(async () => {
    if (!sessionId) return;

    try {
      setError(null);
      
      eventBus.emit(SNAPSHOT_EVENTS.USER_REJECT_SESSION, {}, sessionId);
      await stateManager.handleUserSessionAction(sessionId, 'reject');
      
    } catch (err) {
      log.error('Failed to reject session', { sessionId, error: err });
      setError(t('snapshotSystem.errors.rejectSessionFailed'));
      throw err;
    }
  }, [sessionId, eventBus, stateManager, t]);

  const acceptBlock = useCallback(async (filePath: string, blockId: string) => {
    if (!sessionId) return;

    try {
      setError(null);
      
      eventBus.emit(SNAPSHOT_EVENTS.USER_ACCEPT_BLOCK, { filePath, blockId }, sessionId, filePath);
      await stateManager.handleUserBlockAction(sessionId, filePath, blockId, 'accept');
      
    } catch (err) {
      log.error('Failed to accept block', { sessionId, filePath, blockId, error: err });
      setError(t('snapshotSystem.errors.acceptBlockFailed'));
      throw err;
    }
  }, [sessionId, eventBus, stateManager, t]);

  const rejectBlock = useCallback(async (filePath: string, blockId: string) => {
    if (!sessionId) return;

    try {
      setError(null);
      
      eventBus.emit(SNAPSHOT_EVENTS.USER_REJECT_BLOCK, { filePath, blockId }, sessionId, filePath);
      await stateManager.handleUserBlockAction(sessionId, filePath, blockId, 'reject');
      
    } catch (err) {
      log.error('Failed to reject block', { sessionId, filePath, blockId, error: err });
      setError(t('snapshotSystem.errors.rejectBlockFailed'));
      throw err;
    }
  }, [sessionId, eventBus, stateManager, t]);

  const getCompactDiff = useCallback((filePath: string): CompactDiffResult | null => {
    const file = stateManager.getFileState(filePath);
    if (!file) return null;
    
    return diffEngine.generateCompactDiff(file);
  }, [stateManager, diffEngine]);

  const getFullDiff = useCallback((filePath: string): FullDiffResult | null => {
    const file = stateManager.getFileState(filePath);
    if (!file) return null;
    
    return diffEngine.generateFullDiff(file);
  }, [stateManager, diffEngine]);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  useEffect(() => {
    if (!sessionId) {
      setFiles([]);
      setSessionState(null);
      activeSessionIdRef.current = undefined;
      return;
    }

    activeSessionIdRef.current = sessionId;
    setFiles([]);
    setSessionState(null);

    const unsubscribeSession = stateManager.onSessionStateChange((newSessionState) => {
      if (newSessionState.sessionId === activeSessionIdRef.current) {
        setSessionState(newSessionState);
        setFiles(Array.from(newSessionState.files.values()));
      } else {
        log.debug('Ignoring session state change for different session', { eventSessionId: newSessionState.sessionId, currentSessionId: activeSessionIdRef.current });
      }
    });

    const unsubscribeFile = stateManager.onFileStateChange((file) => {
      if (file.sessionId === activeSessionIdRef.current) {
        setFiles(prev => {
          const newFiles = [...prev];
          const index = newFiles.findIndex(f => f.filePath === file.filePath);
          if (index >= 0) {
            newFiles[index] = file;
          } else {
            newFiles.push(file);
          }
          return newFiles;
        });
      } else {
        log.debug('Ignoring file event for different session', { eventSessionId: file.sessionId, currentSessionId: activeSessionIdRef.current });
      }
    });

    refreshSession();

    return () => {
      unsubscribeSession();
      unsubscribeFile();
    };
  }, [sessionId, stateManager, refreshSession]);

  return {
    sessionState,
    files,
    loading,
    error,
    refreshSession,
    acceptFile,
    rejectFile,
    acceptSession,
    rejectSession,
    acceptBlock,
    rejectBlock,
    getCompactDiff,
    getFullDiff,
    clearError
  };
};
