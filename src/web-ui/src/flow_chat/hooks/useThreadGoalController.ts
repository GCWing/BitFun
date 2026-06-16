import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import { useTranslation } from 'react-i18next';
import { confirmWarning } from '@/component-library/components/ConfirmDialog/confirmService';
import { notificationService } from '@/shared/notification-system';
import type { Session } from '../types/flow-chat';
import { flowChatStore } from '../store/FlowChatStore';
import {
  dismissResumePrompt,
  isResumePromptDismissed,
  threadGoalActionsForStatus,
  threadGoalStatusNeedsResumePrompt,
  type ThreadGoalUiAction,
} from '../services/threadGoalActions';
import { parseGoalCommand } from '../services/goalCommandParser';
import {
  fetchSessionThreadGoal,
  runGoalCommandSafely,
  runThreadGoalUiAction,
  saveThreadGoalObjective,
  type ThreadGoalSnapshot,
} from '../services/goalService';

const HISTORICAL_THREAD_GOAL_REFRESH_DELAY_MS = 350;

export interface ThreadGoalController {
  goal: ThreadGoalSnapshot | null;
  menuOpen: boolean;
  editOpen: boolean;
  editMode: 'create' | 'update';
  editInitialObjective: string;
  resumeOpen: boolean;
  availableActions: ThreadGoalUiAction[];
  openMenu: () => void;
  openEdit: (mode?: 'create' | 'update') => void;
  closeMenu: () => void;
  closeEdit: () => void;
  closeResume: () => void;
  refreshGoal: () => Promise<void>;
  /** Fetch latest goal from backend, then open menu or create dialog. */
  openGoalEntry: () => Promise<void>;
  runSlashAction: (message: string) => Promise<ThreadGoalSnapshot | null>;
  runUiAction: (action: 'clear' | 'pause' | 'resume') => Promise<void>;
  saveEdit: (objective: string) => Promise<void>;
  confirmResume: () => Promise<void>;
  dismissResume: () => void;
}

function readStoreGoal(sessionId: string | undefined): ThreadGoalSnapshot | null {
  if (!sessionId) return null;
  const raw = flowChatStore.getState().sessions.get(sessionId)?.threadGoal;
  if (!raw) return null;
  return {
    goalId: raw.goalId,
    objective: raw.objective,
    status: raw.status,
    tokensUsed: raw.tokensUsed,
    tokenBudget: raw.tokenBudget,
    timeUsedSeconds: raw.timeUsedSeconds,
    updatedAt: raw.updatedAt,
  };
}

function threadGoalSnapshotCacheKey(
  sessionId: string | undefined,
  goal: ThreadGoalSnapshot | null
): string {
  if (!sessionId) return '';
  if (!goal) return `${sessionId}:null`;
  return [
    sessionId,
    goal.goalId ?? '',
    goal.status,
    goal.objective,
    goal.updatedAt ?? '',
    goal.tokensUsed ?? '',
    goal.tokenBudget ?? '',
    goal.timeUsedSeconds ?? '',
  ].join('|');
}

/** useSyncExternalStore requires a stable snapshot reference when store data is unchanged. */
function useStableThreadGoalSnapshot(sessionId: string | undefined): ThreadGoalSnapshot | null {
  const cacheRef = useRef<{ key: string; snapshot: ThreadGoalSnapshot | null }>({
    key: '',
    snapshot: null,
  });

  const subscribe = useCallback(
    (onStoreChange: () => void) => flowChatStore.subscribe(() => onStoreChange()),
    []
  );

  const getSnapshot = useCallback(() => {
    const next = readStoreGoal(sessionId);
    const key = threadGoalSnapshotCacheKey(sessionId, next);
    if (cacheRef.current.key === key) {
      return cacheRef.current.snapshot;
    }
    cacheRef.current = { key, snapshot: next };
    return next;
  }, [sessionId]);

  return useSyncExternalStore(subscribe, getSnapshot, () => null);
}

export function useThreadGoalController(
  session: Session | undefined,
  options?: { isBtwSession?: boolean }
): ThreadGoalController {
  const { t } = useTranslation('flow-chat');
  const sessionId = session?.sessionId;
  const isBtwSession = Boolean(options?.isBtwSession);

  const storeGoal = useStableThreadGoalSnapshot(sessionId);

  const [menuOpen, setMenuOpen] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [editMode, setEditMode] = useState<'create' | 'update'>('update');
  const [editInitialObjective, setEditInitialObjective] = useState('');
  const [resumeOpen, setResumeOpen] = useState(false);
  const lastResumePromptKey = useRef<string | null>(null);

  const goal = storeGoal;

  const titles = useMemo(
    () => ({
      usageMessage: t('chatInput.goalUsage'),
      failedTitle: t('chatInput.goalFailed'),
      unknownErrorMessage: t('error.unknown'),
      activatedTitle: t('chatInput.goalActivated'),
      clearedTitle: t('chatInput.goalCleared'),
      pausedTitle: t('chatInput.goalPaused'),
      resumedTitle: t('chatInput.goalResumed'),
      editedTitle: t('chatInput.goalEdited'),
      replaceConfirmTitle: t('threadGoal.replaceConfirmTitle'),
      replaceConfirmMessage: t('threadGoal.replaceConfirmMessage'),
    }),
    [t]
  );

  const refreshGoal = useCallback(async () => {
    if (!sessionId || isBtwSession) return;
    const current = flowChatStore.getState().sessions.get(sessionId);
    if (!current?.workspacePath) return;
    try {
      await fetchSessionThreadGoal(current);
    } catch {
      // best-effort; UI still works from events
    }
  }, [isBtwSession, sessionId]);

  useEffect(() => {
    if (!sessionId || isBtwSession) return;
    if (session?.isHistorical) {
      const timeoutId = globalThis.setTimeout(() => {
        void refreshGoal();
      }, HISTORICAL_THREAD_GOAL_REFRESH_DELAY_MS);
      return () => globalThis.clearTimeout(timeoutId);
    }
    void refreshGoal();
  }, [session?.isHistorical, sessionId, isBtwSession, refreshGoal]);

  const goalId = goal?.goalId;
  const goalStatus = goal?.status;
  const goalUpdatedAt = goal?.updatedAt;

  useEffect(() => {
    if (!sessionId || !goalId || !goalStatus || !threadGoalStatusNeedsResumePrompt(goalStatus)) {
      return;
    }
    if (!goal || isResumePromptDismissed(sessionId, goal)) {
      return;
    }
    const key = `${goalId}:${goalUpdatedAt ?? 0}:${goalStatus}`;
    if (lastResumePromptKey.current === key) {
      return;
    }
    lastResumePromptKey.current = key;
    setResumeOpen(true);
  }, [goal, goalId, goalStatus, goalUpdatedAt, sessionId]);

  const openMenu = useCallback(() => {
    setMenuOpen(true);
  }, []);

  const confirmReplaceGoal = useCallback(
    async ({ existingObjective, newObjective }: { existingObjective: string; newObjective: string }) =>
      confirmWarning(
        titles.replaceConfirmTitle,
        t('threadGoal.replaceConfirmMessage', {
          existing: existingObjective,
          next: newObjective,
        })
      ),
    [t, titles.replaceConfirmTitle]
  );

  const openEdit = useCallback(
    (mode: 'create' | 'update' = 'update') => {
      setEditMode(mode);
      setEditInitialObjective(mode === 'update' ? (goal?.objective ?? '') : '');
      setEditOpen(true);
      setMenuOpen(false);
    },
    [goal?.objective]
  );

  const openGoalEntry = useCallback(async () => {
    if (!session?.workspacePath || isBtwSession) return;
    const latest = await fetchSessionThreadGoal(session);
    if (latest) {
      setMenuOpen(true);
    } else {
      openEdit('create');
    }
  }, [isBtwSession, openEdit, session]);

  const runSlashAction = useCallback(
    async (message: string) => {
      if (!session) return null;
      const parsed = parseGoalCommand(message);
      if (!parsed) return null;

      return runGoalCommandSafely({
        session,
        action: parsed,
        ...titles,
        confirmReplaceGoal,
        onOpenMenu: g => {
          if (!g) {
            openEdit('create');
            return;
          }
          setMenuOpen(true);
        },
        onOpenEdit: (initial, mode) => {
          setEditInitialObjective(initial);
          setEditMode(mode);
          setEditOpen(true);
        },
      });
    },
    [confirmReplaceGoal, openEdit, session, titles]
  );

  const runUiAction = useCallback(
    async (action: 'clear' | 'pause' | 'resume') => {
      if (!session) return;
      try {
        await runThreadGoalUiAction(session, action, titles);
        if (action === 'clear') {
          setMenuOpen(false);
        }
      } catch (error) {
        const message =
          error instanceof Error && error.message.trim()
            ? error.message.trim()
            : titles.unknownErrorMessage;
        notificationService.error(message, { title: titles.failedTitle, duration: 5000 });
      }
    },
    [session, titles]
  );

  const saveEdit = useCallback(
    async (objective: string) => {
      if (!session) return;
      try {
        const saved = await saveThreadGoalObjective(session, objective, editMode, titles, {
          confirmReplaceGoal: editMode === 'create' ? confirmReplaceGoal : undefined,
        });
        if (!saved) {
          return;
        }
        setEditOpen(false);
        setMenuOpen(false);
      } catch (error) {
        const message =
          error instanceof Error && error.message.trim()
            ? error.message.trim()
            : titles.unknownErrorMessage;
        notificationService.error(message, { title: titles.failedTitle, duration: 5000 });
      }
    },
    [confirmReplaceGoal, editMode, session, titles]
  );

  const confirmResume = useCallback(async () => {
    if (!session) return;
    await runUiAction('resume');
    setResumeOpen(false);
  }, [runUiAction, session]);

  const dismissResume = useCallback(() => {
    if (sessionId && goal) {
      dismissResumePrompt(sessionId, goal);
    }
    setResumeOpen(false);
  }, [goal, sessionId]);

  const closeMenu = useCallback(() => setMenuOpen(false), []);
  const closeEdit = useCallback(() => setEditOpen(false), []);
  const closeResume = useCallback(() => setResumeOpen(false), []);

  const availableActions = useMemo(
    () => (goal ? threadGoalActionsForStatus(goal.status) : []),
    [goal]
  );

  return useMemo(
    () => ({
      goal,
      menuOpen,
      editOpen,
      editMode,
      editInitialObjective,
      resumeOpen,
      availableActions,
      openMenu,
      openGoalEntry,
      openEdit,
      closeMenu,
      closeEdit,
      closeResume,
      refreshGoal,
      runSlashAction,
      runUiAction,
      saveEdit,
      confirmResume,
      dismissResume,
    }),
    [
      availableActions,
      closeEdit,
      closeMenu,
      closeResume,
      confirmResume,
      dismissResume,
      editInitialObjective,
      editMode,
      editOpen,
      goal,
      menuOpen,
      openEdit,
      openGoalEntry,
      openMenu,
      refreshGoal,
      resumeOpen,
      runSlashAction,
      runUiAction,
      saveEdit,
    ]
  );
}
