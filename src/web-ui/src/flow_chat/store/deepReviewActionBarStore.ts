/**
 * Shared review action bar state rendered at the bottom of BtwSessionPanel.
 *
 * The legacy DeepReview exports are intentionally kept as aliases so existing
 * callers can migrate incrementally while standard Code Review starts using
 * the same confirmation surface.
 */

import { create } from 'zustand';
import type {
  CodeReviewRemediationData,
  ReviewRemediationItem,
} from '../utils/codeReviewRemediation';
import {
  buildReviewRemediationItems,
  getDefaultSelectedRemediationIds,
} from '../utils/codeReviewRemediation';
import type { RemediationGroupId } from '../utils/codeReviewReport';
import type { DeepReviewInterruption } from '../utils/deepReviewContinuation';

export type ReviewActionMode = 'standard' | 'deep';

export type ReviewActionPhase =
  | 'idle'
  | 'review_running'
  | 'review_completed'
  | 'fix_running'
  | 'fix_completed'
  | 'fix_failed'
  | 'fix_timeout'
  | 'fix_interrupted'
  | 'review_waiting_capacity'
  | 'review_interrupted'
  | 'resume_blocked'
  | 'resume_running'
  | 'resume_failed'
  | 'review_error';

export type DeepReviewActionPhase = ReviewActionPhase;

export type DeepReviewCapacityQueueStatus =
  | 'queued_for_capacity'
  | 'paused_by_user'
  | 'running'
  | 'capacity_skipped';

export type DeepReviewCapacityQueueAction =
  | 'pause'
  | 'continue'
  | 'cancel'
  | 'skip_optional';

export type DeepReviewCapacityQueueReason =
  | 'provider_rate_limit'
  | 'provider_concurrency_limit'
  | 'retry_after'
  | 'local_concurrency_cap'
  | 'launch_batch_blocked'
  | 'temporary_overload';

export interface DeepReviewCapacityWaitingReviewer {
  toolId?: string;
  subagentType?: string;
  displayName?: string;
  status: Exclude<DeepReviewCapacityQueueStatus, 'running' | 'capacity_skipped'>;
  reason?: DeepReviewCapacityQueueReason;
  optional?: boolean;
  queueElapsedMs?: number;
  maxQueueWaitSeconds?: number;
}

export interface DeepReviewCapacityQueueState {
  toolId?: string;
  subagentType?: string;
  dialogTurnId?: string;
  status: DeepReviewCapacityQueueStatus;
  reason?: DeepReviewCapacityQueueReason;
  queuedReviewerCount: number;
  activeReviewerCount?: number;
  effectiveParallelInstances?: number;
  optionalReviewerCount?: number;
  queueElapsedMs?: number;
  runElapsedMs?: number;
  maxQueueWaitSeconds?: number;
  sessionConcurrencyHigh?: boolean;
  controlMode?: 'local' | 'session_stop_only' | 'backend';
  waitingReviewers?: DeepReviewCapacityWaitingReviewer[];
}

export interface ReviewActionBarData {
  /** Which child session this bar belongs to */
  childSessionId: string | null;
  /** Parent session (used to fill-back the input) */
  parentSessionId: string | null;
  /** Which review mode owns this action bar */
  reviewMode: ReviewActionMode;
  /** Current phase of the review lifecycle */
  phase: ReviewActionPhase;
  /** The raw review result data (remediation plan, issues, etc.) */
  reviewData: CodeReviewRemediationData | null;
  /** Pre-built remediation items derived from reviewData */
  remediationItems: ReviewRemediationItem[];
  /** IDs of the remediation items the user selected */
  selectedRemediationIds: Set<string>;
  /** Whether the action bar is minimized (collapsed to a floating button) */
  minimized: boolean;
  /** Which fix action is currently in flight */
  activeAction: 'fix' | 'fix-review' | 'resume' | 'retry' | null;
  /** Last user action that changed the action bar content */
  lastSubmittedAction: 'fix' | 'fix-review' | 'resume' | 'retry' | null;
  /** User-supplied custom instructions (from the textarea) */
  customInstructions: string;
  /** Error message when phase is fix_failed or review_error */
  errorMessage: string | null;
  /** Structured interruption state used to continue an incomplete Deep Review */
  interruption: DeepReviewInterruption | null;
  /** IDs of remediation items that have been fixed/completed */
  completedRemediationIds: Set<string>;
  /** IDs of items being fixed in the current fix_running session (snapshot at start) */
  fixingRemediationIds: Set<string>;
  /** Last dialog turn that existed before the current fix request was submitted */
  fixingBaselineTurnId: string | null;
  /** Last dialog turn that existed before the current resume request was submitted */
  resumeBaselineTurnId: string | null;
  /** IDs of items remaining when a fix was interrupted */
  remainingFixIds: string[];
  /** User's option choice for needs_decision items: map of item id -> option index */
  decisionSelections: Record<string, number>;
  /** Visible Deep Review capacity queue state. Automatic queue execution is not enabled here. */
  capacityQueueState: DeepReviewCapacityQueueState | null;
  /** Last local queue-control action selected by the user */
  lastCapacityQueueAction: DeepReviewCapacityQueueAction | null;
}

export interface ReviewActionBarState extends ReviewActionBarData {
  /** Per-review action bar states keyed by child session id. */
  sessionStates: Record<string, ReviewActionBarData>;

  // ---- actions ----
  getSessionState: (childSessionId: string | null | undefined) => ReviewActionBarData | null;
  showActionBar: (params: {
    childSessionId: string;
    parentSessionId: string | null;
    reviewData: CodeReviewRemediationData;
    reviewMode?: ReviewActionMode;
    phase?: ReviewActionPhase;
    completedRemediationIds?: Set<string>;
  }) => void;
  showRunningActionBar: (params: {
    childSessionId: string;
    parentSessionId: string | null;
    reviewMode?: ReviewActionMode;
  }) => void;
  showInterruptedActionBar: (params: {
    childSessionId: string;
    parentSessionId: string | null;
    interruption: DeepReviewInterruption;
    phase?: Extract<ReviewActionPhase, 'review_interrupted' | 'resume_blocked' | 'resume_failed'>;
  }) => void;
  showCapacityQueueBar: (params: {
    childSessionId: string;
    parentSessionId: string | null;
    capacityQueueState: DeepReviewCapacityQueueState;
  }) => void;
  updatePhase: (phase: ReviewActionPhase, errorMessage?: string | null, childSessionId?: string) => void;
  toggleRemediation: (id: string, childSessionId?: string) => void;
  toggleAllRemediation: (childSessionId?: string) => void;
  toggleGroupRemediation: (groupId: RemediationGroupId, childSessionId?: string) => void;
  setActiveAction: (
    action: 'fix' | 'fix-review' | 'resume' | 'retry' | null,
    options?: { baselineTurnId?: string | null },
    childSessionId?: string,
  ) => void;
  setCustomInstructions: (value: string, childSessionId?: string) => void;
  setSelectedRemediationIds: (ids: Set<string>, childSessionId?: string) => void;
  minimize: (childSessionId?: string) => void;
  restore: (childSessionId?: string) => void;
  skipRemainingFixes: (childSessionId?: string) => void;
  setCapacityQueueState: (state: DeepReviewCapacityQueueState | null, childSessionId?: string) => void;
  applyCapacityQueueState: (state: DeepReviewCapacityQueueState, childSessionId?: string) => void;
  pauseCapacityQueue: (childSessionId?: string) => void;
  continueCapacityQueue: (childSessionId?: string) => void;
  cancelQueuedReviewers: (childSessionId?: string) => void;
  skipOptionalQueuedReviewers: (childSessionId?: string) => void;
  setDecisionSelection: (itemId: string, optionIndex: number, childSessionId?: string) => void;
  setRemainingFixIds: (ids: string[], childSessionId?: string) => void;
  reset: () => void;
}

export type DeepReviewActionBarState = ReviewActionBarState;

const initialState: ReviewActionBarData = {
  childSessionId: null as string | null,
  parentSessionId: null as string | null,
  reviewMode: 'deep' as ReviewActionMode,
  phase: 'idle' as ReviewActionPhase,
  reviewData: null as CodeReviewRemediationData | null,
  remediationItems: [] as ReviewRemediationItem[],
  selectedRemediationIds: new Set<string>(),
  minimized: false,
  activeAction: null as 'fix' | 'fix-review' | 'resume' | 'retry' | null,
  lastSubmittedAction: null as 'fix' | 'fix-review' | 'resume' | 'retry' | null,
  customInstructions: '',
  errorMessage: null as string | null,
  interruption: null as DeepReviewInterruption | null,
  completedRemediationIds: new Set<string>(),
  fixingRemediationIds: new Set<string>(),
  fixingBaselineTurnId: null as string | null,
  resumeBaselineTurnId: null as string | null,
  remainingFixIds: [] as string[],
  decisionSelections: {} as Record<string, number>,
  capacityQueueState: null as DeepReviewCapacityQueueState | null,
  lastCapacityQueueAction: null as DeepReviewCapacityQueueAction | null,
};

function cloneActionData(data: ReviewActionBarData): ReviewActionBarData {
  return {
    ...data,
    remediationItems: [...data.remediationItems],
    selectedRemediationIds: new Set(data.selectedRemediationIds),
    completedRemediationIds: new Set(data.completedRemediationIds),
    fixingRemediationIds: new Set(data.fixingRemediationIds),
    remainingFixIds: [...data.remainingFixIds],
    decisionSelections: { ...data.decisionSelections },
    capacityQueueState: data.capacityQueueState
      ? withNormalizedWaitingReviewers(data.capacityQueueState)
      : null,
  };
}

function createInitialActionData(): ReviewActionBarData {
  return cloneActionData(initialState);
}

function snapshotActionData(state: ReviewActionBarData): ReviewActionBarData {
  return cloneActionData(state);
}

export function getReviewActionBarStateForSession(
  state: ReviewActionBarState,
  childSessionId: string | null | undefined,
): ReviewActionBarData | null {
  if (!childSessionId) {
    return null;
  }

  return state.childSessionId === childSessionId
    ? state
    : state.sessionStates[childSessionId] ?? null;
}

function isTerminalQueueStatus(status: DeepReviewCapacityQueueStatus): boolean {
  return status === 'running' || status === 'capacity_skipped';
}

function queueReviewerKey(
  reviewer: Pick<DeepReviewCapacityWaitingReviewer, 'toolId' | 'subagentType'>,
): string {
  return reviewer.toolId || reviewer.subagentType || 'unknown-reviewer';
}

function waitingReviewerFromQueueState(
  state: DeepReviewCapacityQueueState,
): DeepReviewCapacityWaitingReviewer | null {
  if (state.status === 'running' || state.status === 'capacity_skipped') {
    return null;
  }

  return {
    toolId: state.toolId,
    subagentType: state.subagentType,
    status: state.status,
    reason: state.reason,
    optional: (state.optionalReviewerCount ?? 0) > 0,
    queueElapsedMs: state.queueElapsedMs,
    maxQueueWaitSeconds: state.maxQueueWaitSeconds,
  };
}

function normalizeWaitingReviewers(
  state: DeepReviewCapacityQueueState,
): DeepReviewCapacityWaitingReviewer[] {
  if (state.waitingReviewers) {
    return state.waitingReviewers;
  }

  const reviewer = waitingReviewerFromQueueState(state);
  return reviewer ? [reviewer] : [];
}

function withNormalizedWaitingReviewers(
  state: DeepReviewCapacityQueueState,
): DeepReviewCapacityQueueState {
  return {
    ...state,
    waitingReviewers: normalizeWaitingReviewers(state),
  };
}

function mergeCapacityQueueState(
  current: DeepReviewCapacityQueueState | null,
  incoming: DeepReviewCapacityQueueState,
): DeepReviewCapacityQueueState | null {
  const currentReviewers = current?.waitingReviewers ?? normalizeWaitingReviewers(current ?? incoming);
  const reviewerMap = new Map(
    currentReviewers.map((reviewer) => [queueReviewerKey(reviewer), reviewer]),
  );
  const incomingReviewers = normalizeWaitingReviewers(incoming);
  const fallbackIncomingKey = queueReviewerKey(incoming);

  if (isTerminalQueueStatus(incoming.status)) {
    reviewerMap.delete(fallbackIncomingKey);
    for (const reviewer of incomingReviewers) {
      reviewerMap.delete(queueReviewerKey(reviewer));
    }
  } else {
    for (const reviewer of incomingReviewers) {
      reviewerMap.set(queueReviewerKey(reviewer), reviewer);
    }
  }

  const waitingReviewers = [...reviewerMap.values()];
  if (waitingReviewers.length === 0) {
    if (isTerminalQueueStatus(incoming.status) && incoming.queuedReviewerCount > 0) {
      return {
        ...incoming,
        status: current?.status === 'paused_by_user' ? 'paused_by_user' : 'queued_for_capacity',
        queuedReviewerCount: incoming.queuedReviewerCount,
        optionalReviewerCount: incoming.optionalReviewerCount ?? 0,
        waitingReviewers: [],
      };
    }

    return null;
  }

  const queuedReviewerCount = Math.max(waitingReviewers.length, incoming.queuedReviewerCount ?? 0);
  const optionalReviewerCount = waitingReviewers.filter((reviewer) => reviewer.optional).length;
  const allPaused = waitingReviewers.every((reviewer) => reviewer.status === 'paused_by_user');

  return {
    ...incoming,
    status: allPaused ? 'paused_by_user' : 'queued_for_capacity',
    queuedReviewerCount,
    optionalReviewerCount,
    waitingReviewers,
  };
}

export const useReviewActionBarStore = create<ReviewActionBarState>((set, get) => {
  const readSessionData = (childSessionId: string | null | undefined): ReviewActionBarData | null => {
    if (!childSessionId) {
      return null;
    }

    return getReviewActionBarStateForSession(get(), childSessionId);
  };

  const readTargetData = (childSessionId?: string): ReviewActionBarData => {
    const targetSessionId = childSessionId ?? get().childSessionId;
    if (!targetSessionId) {
      return snapshotActionData(get());
    }

    const sessionData = readSessionData(targetSessionId);
    return sessionData
      ? snapshotActionData(sessionData)
      : {
          ...createInitialActionData(),
          childSessionId: targetSessionId,
        };
  };

  const commitActionData = (data: ReviewActionBarData): void => {
    const next = snapshotActionData(data);
    set((state) => ({
      ...next,
      sessionStates: next.childSessionId
        ? {
            ...state.sessionStates,
            [next.childSessionId]: next,
          }
        : state.sessionStates,
    }));
  };

  const updateActionData = (
    childSessionId: string | undefined,
    updater: (current: ReviewActionBarData) => ReviewActionBarData,
  ): void => {
    commitActionData(updater(readTargetData(childSessionId)));
  };

  return ({
  ...initialState,
  sessionStates: {},

  getSessionState: (childSessionId) => readSessionData(childSessionId),

  showActionBar: ({ childSessionId, parentSessionId, reviewData, reviewMode, phase, completedRemediationIds }) => {
    const items = buildReviewRemediationItems(reviewData);
    const defaultIds = new Set(getDefaultSelectedRemediationIds(items));

    // If completedRemediationIds is provided, filter out items that no longer exist
    const existingIds = new Set(items.map((i) => i.id));
    const preservedCompleted = completedRemediationIds
      ? new Set([...completedRemediationIds].filter((id) => existingIds.has(id)))
      : new Set<string>();

    // Remove completed items from default selection
    for (const id of preservedCompleted) {
      defaultIds.delete(id);
    }

    commitActionData({
      childSessionId,
      parentSessionId,
      reviewMode: reviewMode ?? reviewData.review_mode ?? 'deep',
      reviewData,
      remediationItems: items,
      selectedRemediationIds: defaultIds,
      phase: phase ?? 'review_completed',
      minimized: false,
      activeAction: null,
      lastSubmittedAction: null,
      customInstructions: '',
      errorMessage: null,
      interruption: null,
      completedRemediationIds: preservedCompleted,
      fixingRemediationIds: new Set(),
      fixingBaselineTurnId: null,
      resumeBaselineTurnId: null,
      remainingFixIds: [],
      decisionSelections: {},
      capacityQueueState: null,
      lastCapacityQueueAction: null,
    });
  },

  showRunningActionBar: ({ childSessionId, parentSessionId, reviewMode }) => {
    commitActionData({
      childSessionId,
      parentSessionId,
      reviewMode: reviewMode ?? 'deep',
      reviewData: null,
      remediationItems: [],
      selectedRemediationIds: new Set(),
      phase: 'review_running',
      minimized: true,
      activeAction: null,
      lastSubmittedAction: null,
      customInstructions: '',
      errorMessage: null,
      interruption: null,
      completedRemediationIds: new Set(),
      fixingRemediationIds: new Set(),
      fixingBaselineTurnId: null,
      resumeBaselineTurnId: null,
      remainingFixIds: [],
      decisionSelections: {},
      capacityQueueState: null,
      lastCapacityQueueAction: null,
    });
  },

  showInterruptedActionBar: ({ childSessionId, parentSessionId, interruption, phase }) => {
    commitActionData({
      childSessionId,
      parentSessionId,
      reviewMode: 'deep',
      reviewData: null,
      remediationItems: [],
      selectedRemediationIds: new Set(),
      phase: phase ?? interruption.phase,
      minimized: false,
      activeAction: null,
      lastSubmittedAction: null,
      customInstructions: '',
      errorMessage: null,
      interruption,
      completedRemediationIds: new Set(),
      fixingRemediationIds: new Set(),
      fixingBaselineTurnId: null,
      resumeBaselineTurnId: null,
      remainingFixIds: [],
      decisionSelections: {},
      capacityQueueState: null,
      lastCapacityQueueAction: null,
    });
  },

  showCapacityQueueBar: ({ childSessionId, parentSessionId, capacityQueueState }) => {
    const current = readSessionData(childSessionId);
    commitActionData({
      childSessionId,
      parentSessionId,
      reviewMode: 'deep',
      reviewData: null,
      remediationItems: [],
      selectedRemediationIds: new Set(),
      phase: 'review_waiting_capacity',
      minimized: false,
      activeAction: null,
      lastSubmittedAction: null,
      customInstructions: '',
      errorMessage: null,
      interruption: null,
      completedRemediationIds: current
        ? new Set(current.completedRemediationIds)
        : new Set(),
      fixingRemediationIds: new Set(),
      fixingBaselineTurnId: null,
      resumeBaselineTurnId: null,
      remainingFixIds: [],
      decisionSelections: {},
      capacityQueueState: withNormalizedWaitingReviewers(capacityQueueState),
      lastCapacityQueueAction: null,
    });
  },

  updatePhase: (phase, errorMessage, childSessionId) => {
    const current = readTargetData(childSessionId);
    const prevPhase = current.phase;
    if (prevPhase === 'fix_running' && phase === 'fix_completed') {
      const { fixingRemediationIds, completedRemediationIds } = current;
      const nextCompleted = new Set(completedRemediationIds);
      for (const id of fixingRemediationIds) {
        nextCompleted.add(id);
      }
      commitActionData({
        ...current,
        phase,
        errorMessage: errorMessage ?? null,
        completedRemediationIds: nextCompleted,
        fixingRemediationIds: new Set(),
        fixingBaselineTurnId: null,
        resumeBaselineTurnId: null,
        remainingFixIds: [],
      });
    } else {
      commitActionData({
        ...current,
        phase,
        errorMessage: errorMessage ?? null,
        ...(phase !== 'fix_running' ? { fixingBaselineTurnId: null } : {}),
        ...(phase !== 'resume_running' ? { resumeBaselineTurnId: null } : {}),
      });
    }
  },

  toggleRemediation: (id, childSessionId) => {
    const current = readTargetData(childSessionId);
    const { completedRemediationIds, selectedRemediationIds } = current;
    if (completedRemediationIds.has(id)) {
      return;
    }

    const next = new Set(selectedRemediationIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    commitActionData({ ...current, selectedRemediationIds: next });
  },

  toggleAllRemediation: (childSessionId) => {
    const current = readTargetData(childSessionId);
    const { remediationItems, selectedRemediationIds, completedRemediationIds } = current;
    const selectableIds = remediationItems
      .filter((item) => !completedRemediationIds.has(item.id))
      .map((item) => item.id);
    const allSelected = selectableIds.length > 0 &&
      selectableIds.every((id) => selectedRemediationIds.has(id));
    const next = new Set(selectedRemediationIds);

    for (const id of completedRemediationIds) {
      next.delete(id);
    }

    if (allSelected) {
      for (const id of selectableIds) {
        next.delete(id);
      }
    } else {
      for (const id of selectableIds) {
        next.add(id);
      }
    }

    commitActionData({ ...current, selectedRemediationIds: next });
  },

  toggleGroupRemediation: (groupId, childSessionId) => {
    const current = readTargetData(childSessionId);
    const { remediationItems, selectedRemediationIds, completedRemediationIds } = current;
    const groupIds = new Set(
      remediationItems
        .filter((item) => (item.groupId ?? 'ungrouped') === groupId && !completedRemediationIds.has(item.id))
        .map((item) => item.id),
    );
    if (groupIds.size === 0) return;

    const allGroupSelected = [...groupIds].every((id) => selectedRemediationIds.has(id));
    const next = new Set(selectedRemediationIds);

    for (const id of completedRemediationIds) {
      next.delete(id);
    }

    if (allGroupSelected) {
      for (const id of groupIds) {
        next.delete(id);
      }
    } else {
      for (const id of groupIds) {
        next.add(id);
      }
    }

    commitActionData({ ...current, selectedRemediationIds: next });
  },

  setActiveAction: (action, options, childSessionId) => {
    const current = readTargetData(childSessionId);
    if (action === 'fix' || action === 'fix-review') {
      commitActionData({
        ...current,
        activeAction: action,
        lastSubmittedAction: action,
        fixingRemediationIds: new Set(current.selectedRemediationIds),
        fixingBaselineTurnId: options?.baselineTurnId ?? null,
      });
    } else if (action === 'resume' || action === 'retry') {
      commitActionData({
        ...current,
        activeAction: action,
        lastSubmittedAction: action,
        ...(action === 'resume'
          ? { resumeBaselineTurnId: options?.baselineTurnId ?? null }
          : {}),
      });
    } else {
      commitActionData({ ...current, activeAction: action });
    }
  },
  setCustomInstructions: (value, childSessionId) =>
    updateActionData(childSessionId, (current) => ({ ...current, customInstructions: value })),
  setSelectedRemediationIds: (ids, childSessionId) =>
    updateActionData(childSessionId, (current) => ({ ...current, selectedRemediationIds: ids })),
  minimize: (childSessionId) =>
    updateActionData(childSessionId, (current) => ({ ...current, minimized: true })),
  restore: (childSessionId) =>
    updateActionData(childSessionId, (current) => ({ ...current, minimized: false })),
  setDecisionSelection: (itemId, optionIndex, childSessionId) =>
    updateActionData(childSessionId, (current) => ({
      ...current,
      decisionSelections: { ...current.decisionSelections, [itemId]: optionIndex },
    })),
  setRemainingFixIds: (ids, childSessionId) =>
    updateActionData(childSessionId, (current) => ({ ...current, remainingFixIds: ids })),
  skipRemainingFixes: (childSessionId) =>
    updateActionData(childSessionId, (current) => ({
      ...current,
      phase: 'review_completed',
      remainingFixIds: [],
      fixingBaselineTurnId: null,
      resumeBaselineTurnId: null,
      activeAction: null,
      lastSubmittedAction: null,
    })),
  setCapacityQueueState: (capacityQueueState, childSessionId) =>
    updateActionData(childSessionId, (current) => ({
      ...current,
      capacityQueueState: capacityQueueState
        ? withNormalizedWaitingReviewers(capacityQueueState)
        : null,
      lastCapacityQueueAction: null,
    })),
  applyCapacityQueueState: (capacityQueueState, childSessionId) => {
    const current = readTargetData(childSessionId);
    const nextQueueState = mergeCapacityQueueState(current.capacityQueueState, capacityQueueState);
    commitActionData({
      ...current,
      capacityQueueState: nextQueueState,
      lastCapacityQueueAction: null,
      ...(nextQueueState === null && current.phase === 'review_waiting_capacity'
        ? { phase: 'idle' as ReviewActionPhase }
        : {}),
    });
  },
  pauseCapacityQueue: (childSessionId) => {
    const currentAction = readTargetData(childSessionId);
    const current = currentAction.capacityQueueState;
    if (!current || current.status === 'capacity_skipped') return;
    commitActionData({
      ...currentAction,
      capacityQueueState: {
        ...current,
        status: 'paused_by_user',
        waitingReviewers: current.waitingReviewers?.map((reviewer) => ({
          ...reviewer,
          status: 'paused_by_user',
        })),
      },
      lastCapacityQueueAction: 'pause',
    });
  },
  continueCapacityQueue: (childSessionId) => {
    const currentAction = readTargetData(childSessionId);
    const current = currentAction.capacityQueueState;
    if (!current || current.status !== 'paused_by_user') return;
    commitActionData({
      ...currentAction,
      capacityQueueState: {
        ...current,
        status: 'queued_for_capacity',
        waitingReviewers: current.waitingReviewers?.map((reviewer) => ({
          ...reviewer,
          status: 'queued_for_capacity',
        })),
      },
      lastCapacityQueueAction: 'continue',
    });
  },
  cancelQueuedReviewers: (childSessionId) => {
    const currentAction = readTargetData(childSessionId);
    const current = currentAction.capacityQueueState;
    if (!current) return;
    commitActionData({
      ...currentAction,
      capacityQueueState: {
        ...current,
        status: 'capacity_skipped',
        queuedReviewerCount: 0,
        optionalReviewerCount: 0,
        waitingReviewers: [],
      },
      lastCapacityQueueAction: 'cancel',
    });
  },
  skipOptionalQueuedReviewers: (childSessionId) => {
    const currentAction = readTargetData(childSessionId);
    const current = currentAction.capacityQueueState;
    if (!current) return;
    const optionalCount = current.optionalReviewerCount ?? 0;
    if (optionalCount <= 0) return;

    const skippedCount = Math.min(optionalCount, current.queuedReviewerCount);
    const queuedReviewerCount = Math.max(0, current.queuedReviewerCount - skippedCount);
    commitActionData({
      ...currentAction,
      capacityQueueState: {
        ...current,
        status: queuedReviewerCount > 0 ? current.status : 'capacity_skipped',
        queuedReviewerCount,
        optionalReviewerCount: 0,
        waitingReviewers: current.waitingReviewers?.filter((reviewer) => !reviewer.optional),
      },
      lastCapacityQueueAction: 'skip_optional',
    });
  },
  reset: () => set({ ...createInitialActionData(), sessionStates: {} }),
  });
});

// Subscribe to state changes and persist when relevant fields change
let persistTimer: ReturnType<typeof setTimeout> | null = null;
const PERSIST_DEBOUNCE_MS = 1000;

useReviewActionBarStore.subscribe((state, prevState) => {
  if (!state.childSessionId) return;

  const shouldPersist =
    state.phase !== prevState.phase ||
    state.minimized !== prevState.minimized ||
    state.completedRemediationIds !== prevState.completedRemediationIds ||
    state.customInstructions !== prevState.customInstructions;

  if (!shouldPersist) return;

  if (persistTimer) clearTimeout(persistTimer);

  persistTimer = setTimeout(() => {
    import('../services/ReviewActionBarPersistenceService').then(({ persistReviewActionState }) => {
      persistReviewActionState(state).catch(() => {
        // Silently ignore persistence errors
      });
    });
  }, PERSIST_DEBOUNCE_MS);
});

export const useDeepReviewActionBarStore = useReviewActionBarStore;
