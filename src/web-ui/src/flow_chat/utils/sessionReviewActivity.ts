import { SessionExecutionState } from '../state-machine/types';
import type { DialogTurn, FlowChatState, Session } from '../types/flow-chat';

export type SessionReviewActivityKind = 'review' | 'deep_review';
export type SessionReviewActivityLifecycle =
  | 'running'
  | 'finishing'
  | 'completed'
  | 'cancelled'
  | 'error'
  | 'idle';

export interface SessionReviewActivity {
  parentSessionId: string;
  childSessionId: string;
  kind: SessionReviewActivityKind;
  lifecycle: SessionReviewActivityLifecycle;
  isBlocking: boolean;
  startedAt: number;
  updatedAt: number;
}

export type SessionExecutionStateResolver = (
  sessionId: string,
) => SessionExecutionState | undefined;

const BLOCKING_LIFECYCLES = new Set<SessionReviewActivityLifecycle>([
  'running',
  'finishing',
]);

function deriveLifecycleFromTurn(
  turn?: DialogTurn,
  session?: Session,
): SessionReviewActivityLifecycle {
  if (session?.error) {
    return 'error';
  }

  switch (turn?.status) {
    case 'pending':
    case 'image_analyzing':
    case 'processing':
      return 'running';
    case 'finishing':
      return 'finishing';
    case 'cancelled':
      return 'cancelled';
    case 'error':
      return 'error';
    case 'completed':
      return 'completed';
    default:
      return 'idle';
  }
}

function deriveLifecycle(
  session: Session,
  executionState?: SessionExecutionState,
): SessionReviewActivityLifecycle {
  if (session.error) {
    return 'error';
  }

  switch (executionState) {
    case SessionExecutionState.PROCESSING:
      return 'running';
    case SessionExecutionState.FINISHING:
      return 'finishing';
    case SessionExecutionState.ERROR:
      return 'error';
    case SessionExecutionState.IDLE:
    default:
      return deriveLifecycleFromTurn(
        session.dialogTurns[session.dialogTurns.length - 1],
        session,
      );
  }
}

function toReviewActivity(
  session: Session,
  parentSessionId: string,
  resolveExecutionState?: SessionExecutionStateResolver,
): SessionReviewActivity | null {
  const kind = session.sessionKind === 'deep_review'
    ? 'deep_review'
    : session.sessionKind === 'review'
      ? 'review'
      : null;
  if (
    !kind ||
    session.parentSessionId !== parentSessionId
  ) {
    return null;
  }

  const lifecycle = deriveLifecycle(
    session,
    resolveExecutionState?.(session.sessionId),
  );

  return {
    parentSessionId,
    childSessionId: session.sessionId,
    kind,
    lifecycle,
    isBlocking: BLOCKING_LIFECYCLES.has(lifecycle),
    startedAt: session.createdAt,
    updatedAt: session.lastActiveAt || session.updatedAt || session.createdAt,
  };
}

export function isReviewActivityBlocking(
  activity?: SessionReviewActivity | null,
): boolean {
  return Boolean(activity?.isBlocking);
}

export function deriveSessionReviewActivity(
  state: FlowChatState,
  parentSessionId?: string | null,
  resolveExecutionState?: SessionExecutionStateResolver,
): SessionReviewActivity | null {
  if (!parentSessionId) {
    return null;
  }

  const activities = Array.from(state.sessions.values())
    .map(session => toReviewActivity(session, parentSessionId, resolveExecutionState))
    .filter((activity): activity is SessionReviewActivity => Boolean(activity));

  const blockingActivities = activities.filter(activity => activity.isBlocking);
  const candidates = blockingActivities.length > 0 ? blockingActivities : activities;

  return candidates.sort((left, right) => {
    const updatedDelta = right.updatedAt - left.updatedAt;
    return updatedDelta !== 0 ? updatedDelta : right.startedAt - left.startedAt;
  })[0] ?? null;
}
