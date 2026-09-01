import { useState, useEffect, useCallback, useRef, useMemo, useSyncExternalStore } from 'react';
import { useTranslation } from 'react-i18next';
import { SnapshotStateManager, SessionState, SnapshotFile } from '../core/SnapshotStateManager';
import { SnapshotEventBus, SNAPSHOT_EVENTS } from '../core/SnapshotEventBus';
import { DiffDisplayEngine, CompactDiffResult, FullDiffResult } from '../core/DiffDisplayEngine';
import SnapshotLazyLoader from '../core/SnapshotLazyLoader';
import { createLogger } from '@/shared/utils/logger';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { hasSessionFileSnapshots, shouldRefreshSnapshotForSession } from './snapshotRefreshPolicy';
import { getActiveSurfaceScope, onSurfaceActivated } from '@/infrastructure/peer-device/deviceSurface';

const log = createLogger('useSnapshotState');
const subscribeToSurfaceActivation = (listener: () => void): (() => void) =>
  onSurfaceActivated(() => listener());

interface UseSnapshotStateReturn {
  surfaceEpoch: number;
  snapshotsAvailable: boolean;
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
  const surfaceScope = useSyncExternalStore(
    subscribeToSurfaceActivation, getActiveSurfaceScope, getActiveSurfaceScope,
  );
  const identity = surfaceScope.key(surfaceScope.epoch, sessionId);
  const [boundIdentity, setBoundIdentity] = useState(identity);
  const snapshotsAvailable = useSyncExternalStore(
    (callback) => flowChatStore.subscribe(() => callback()),
    () => hasSessionFileSnapshots(sessionId ? flowChatStore.getState().sessions.get(sessionId) : undefined),
    () => false,
  );
  const [sessionState, setSessionState] = useState<SessionState | null>(null);
  const [files, setFiles] = useState<SnapshotFile[]>([]);
  const unavailableFiles = useMemo<SnapshotFile[]>(() => [], []);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Track the active session to avoid applying stale events after session switches.
  const activeSessionIdRef = useRef<string | undefined>(sessionId);
  const refreshGenerationRef = useRef(0);

  const stateManager = SnapshotStateManager.getInstance();
  const eventBus = SnapshotEventBus.getInstance();
  const diffEngine = useMemo(() => new DiffDisplayEngine(), []);

  const refreshSession = useCallback(async () => {
    if (!sessionId || !surfaceScope.isCurrent()) return;

    const session = flowChatStore.getState().sessions.get(sessionId);
    if (!shouldRefreshSnapshotForSession(session)) {
      refreshGenerationRef.current += 1;
      setLoading(false);
      setError(null);
      setSessionState(null);
      setFiles([]);
      return;
    }

    const refreshGeneration = refreshGenerationRef.current + 1;
    refreshGenerationRef.current = refreshGeneration;

    setLoading(true);
    setError(null);
    
    try {
      await SnapshotLazyLoader.ensureInitialized();

      if (
        !surfaceScope.isCurrent() ||
        refreshGenerationRef.current !== refreshGeneration ||
        activeSessionIdRef.current !== sessionId ||
        !shouldRefreshSnapshotForSession(flowChatStore.getState().sessions.get(sessionId))
      ) {
        return;
      }
      
      await stateManager.refreshSessionState(sessionId);

      if (
        !surfaceScope.isCurrent() ||
        refreshGenerationRef.current !== refreshGeneration ||
        activeSessionIdRef.current !== sessionId ||
        !shouldRefreshSnapshotForSession(flowChatStore.getState().sessions.get(sessionId))
      ) {
        return;
      }

      const newSessionState = stateManager.getSessionState(sessionId);
      const newFiles = stateManager.getSessionFiles(sessionId);
      
      setSessionState(newSessionState);
      setFiles(newFiles);
    } catch (err) {
      if (surfaceScope.isCurrent() && refreshGenerationRef.current === refreshGeneration && activeSessionIdRef.current === sessionId) {
        log.error('Failed to refresh session state', { sessionId, error: err });
        setError(t('snapshotSystem.errors.refreshSessionFailed'));
      }
    } finally {
      if (surfaceScope.isCurrent() && refreshGenerationRef.current === refreshGeneration && activeSessionIdRef.current === sessionId) {
        setLoading(false);
      }
    }
  }, [sessionId, stateManager, surfaceScope, t]);

  const acceptFile = useCallback(async (filePath: string) => {
    if (!sessionId || !surfaceScope.isCurrent()) return;

    try {
      setError(null);
      
      await SnapshotLazyLoader.ensureInitialized();
      if (!surfaceScope.isCurrent()) return;
      
      eventBus.emit(SNAPSHOT_EVENTS.USER_ACCEPT_FILE, { filePath }, sessionId, filePath);
      
      await stateManager.handleUserFileAction(sessionId, filePath, 'accept');
      
    } catch (err) {
      if (!surfaceScope.isCurrent()) return;
      log.error('Failed to accept file', { sessionId, filePath, error: err });
      setError(t('snapshotSystem.errors.acceptFileFailed'));
      throw err;
    }
  }, [sessionId, eventBus, stateManager, surfaceScope, t]);

  const rejectFile = useCallback(async (filePath: string) => {
    if (!sessionId || !surfaceScope.isCurrent()) return;

    try {
      setError(null);
      
      eventBus.emit(SNAPSHOT_EVENTS.USER_REJECT_FILE, { filePath }, sessionId, filePath);
      
      await stateManager.handleUserFileAction(sessionId, filePath, 'reject');
      
    } catch (err) {
      if (!surfaceScope.isCurrent()) return;
      log.error('Failed to reject file', { sessionId, filePath, error: err });
      setError(t('snapshotSystem.errors.rejectFileFailed'));
      throw err;
    }
  }, [sessionId, eventBus, stateManager, surfaceScope, t]);

  const acceptSession = useCallback(async () => {
    if (!sessionId || !surfaceScope.isCurrent()) return;

    try {
      setError(null);
      
      eventBus.emit(SNAPSHOT_EVENTS.USER_ACCEPT_SESSION, {}, sessionId);
      await stateManager.handleUserSessionAction(sessionId, 'accept');
      
    } catch (err) {
      if (!surfaceScope.isCurrent()) return;
      log.error('Failed to accept session', { sessionId, error: err });
      setError(t('snapshotSystem.errors.acceptSessionFailed'));
      throw err;
    }
  }, [sessionId, eventBus, stateManager, surfaceScope, t]);

  const rejectSession = useCallback(async () => {
    if (!sessionId || !surfaceScope.isCurrent()) return;

    try {
      setError(null);
      
      eventBus.emit(SNAPSHOT_EVENTS.USER_REJECT_SESSION, {}, sessionId);
      await stateManager.handleUserSessionAction(sessionId, 'reject');
      
    } catch (err) {
      if (!surfaceScope.isCurrent()) return;
      log.error('Failed to reject session', { sessionId, error: err });
      setError(t('snapshotSystem.errors.rejectSessionFailed'));
      throw err;
    }
  }, [sessionId, eventBus, stateManager, surfaceScope, t]);

  const acceptBlock = useCallback(async (filePath: string, blockId: string) => {
    if (!sessionId || !surfaceScope.isCurrent()) return;

    try {
      setError(null);
      
      eventBus.emit(SNAPSHOT_EVENTS.USER_ACCEPT_BLOCK, { filePath, blockId }, sessionId, filePath);
      await stateManager.handleUserBlockAction(sessionId, filePath, blockId, 'accept');
      
    } catch (err) {
      if (!surfaceScope.isCurrent()) return;
      log.error('Failed to accept block', { sessionId, filePath, blockId, error: err });
      setError(t('snapshotSystem.errors.acceptBlockFailed'));
      throw err;
    }
  }, [sessionId, eventBus, stateManager, surfaceScope, t]);

  const rejectBlock = useCallback(async (filePath: string, blockId: string) => {
    if (!sessionId || !surfaceScope.isCurrent()) return;

    try {
      setError(null);
      
      eventBus.emit(SNAPSHOT_EVENTS.USER_REJECT_BLOCK, { filePath, blockId }, sessionId, filePath);
      await stateManager.handleUserBlockAction(sessionId, filePath, blockId, 'reject');
      
    } catch (err) {
      if (!surfaceScope.isCurrent()) return;
      log.error('Failed to reject block', { sessionId, filePath, blockId, error: err });
      setError(t('snapshotSystem.errors.rejectBlockFailed'));
      throw err;
    }
  }, [sessionId, eventBus, stateManager, surfaceScope, t]);

  const getCompactDiff = useCallback((filePath: string): CompactDiffResult | null => {
    if (!surfaceScope.isCurrent()) return null;
    if (!hasSessionFileSnapshots(sessionId ? flowChatStore.getState().sessions.get(sessionId) : undefined)) return null;
    const file = stateManager.getFileState(filePath);
    if (!file) return null;
    
    return diffEngine.generateCompactDiff(file);
  }, [sessionId, stateManager, diffEngine, surfaceScope]);

  const getFullDiff = useCallback((filePath: string): FullDiffResult | null => {
    if (!surfaceScope.isCurrent()) return null;
    if (!hasSessionFileSnapshots(sessionId ? flowChatStore.getState().sessions.get(sessionId) : undefined)) return null;
    const file = stateManager.getFileState(filePath);
    if (!file) return null;
    
    return diffEngine.generateFullDiff(file);
  }, [sessionId, stateManager, diffEngine, surfaceScope]);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  useEffect(() => {
    setBoundIdentity(identity);
    setLoading(false);
    setError(null);
    if (!sessionId) {
      setFiles([]);
      setSessionState(null);
      activeSessionIdRef.current = undefined;
      return;
    }

    activeSessionIdRef.current = sessionId;
    refreshGenerationRef.current += 1;
    setFiles([]);
    setSessionState(null);

    const unsubscribeSession = stateManager.onSessionStateChange((newSessionState) => {
      if (surfaceScope.isCurrent() && newSessionState.sessionId === activeSessionIdRef.current &&
        shouldRefreshSnapshotForSession(flowChatStore.getState().sessions.get(sessionId))) {
        setSessionState(newSessionState);
        setFiles(Array.from(newSessionState.files.values()));
      } else {
        log.debug('Ignoring session state change for different session', { eventSessionId: newSessionState.sessionId, currentSessionId: activeSessionIdRef.current });
      }
    });

    const unsubscribeFile = stateManager.onFileStateChange((file) => {
      if (surfaceScope.isCurrent() && file.sessionId === activeSessionIdRef.current &&
        shouldRefreshSnapshotForSession(flowChatStore.getState().sessions.get(sessionId))) {
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

    let canRefresh = shouldRefreshSnapshotForSession(
      flowChatStore.getState().sessions.get(sessionId)
    );

    if (canRefresh) {
      refreshSession();
    }

    const unsubscribeFlowChat = flowChatStore.subscribe((state) => {
      if (!surfaceScope.isCurrent()) return;
      const nextCanRefresh = shouldRefreshSnapshotForSession(state.sessions.get(sessionId));
      if (nextCanRefresh && !canRefresh) {
        canRefresh = true;
        refreshSession();
        return;
      }

      if (!nextCanRefresh && canRefresh) {
        canRefresh = false;
        refreshGenerationRef.current += 1;
        setLoading(false);
        setError(null);
        setFiles([]);
        setSessionState(null);
      }
    });

    return () => {
      refreshGenerationRef.current += 1;
      unsubscribeSession();
      unsubscribeFile();
      unsubscribeFlowChat();
    };
  }, [identity, sessionId, stateManager, refreshSession, surfaceScope]);

  // An activation may render the same Session id before effects rebind. Never
  // paint the previous device's cached files during that render.
  const hasCurrentState = snapshotsAvailable && boundIdentity === identity;
  return {
    surfaceEpoch: surfaceScope.epoch,
    snapshotsAvailable,
    sessionState: hasCurrentState ? sessionState : null,
    files: hasCurrentState ? files : unavailableFiles,
    loading: hasCurrentState && loading,
    error: hasCurrentState ? error : null,
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
