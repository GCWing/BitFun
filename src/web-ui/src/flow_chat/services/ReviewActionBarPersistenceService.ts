/**
 * Review Action Bar persistence service.
 *
 * Persists review action bar state to session metadata via the backend API,
 * aligning with the existing session persistence architecture.
 */

import { createLogger } from '@/shared/utils/logger';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { flowChatStore } from '../store/FlowChatStore';
import { buildSessionMetadata } from '../utils/sessionMetadata';
import type { ReviewActionBarState } from '../store/deepReviewActionBarStore';
import type { ReviewActionPersistedState, SessionMetadata } from '@/shared/types/session-history';

const log = createLogger('ReviewActionBarPersistence');

export async function persistReviewActionState(state: ReviewActionBarState): Promise<void> {
  if (!state.childSessionId) return;

  const session = flowChatStore.getState().sessions.get(state.childSessionId);
  if (!session?.workspacePath) return;

  const payload: ReviewActionPersistedState = {
    version: 1,
    phase: state.phase,
    completedRemediationIds: [...state.completedRemediationIds],
    minimized: state.minimized,
    customInstructions: state.customInstructions,
    persistedAt: Date.now(),
  };

  try {
    let existingMetadata: SessionMetadata | null = null;
    try {
      existingMetadata = await sessionAPI.loadSessionMetadata(
        state.childSessionId,
        session.workspacePath,
        session.remoteConnectionId,
        session.remoteSshHost
      );
    } catch (error) {
      log.warn('Failed to load session metadata before persisting review action state', {
        sessionId: state.childSessionId,
        error,
      });
    }

    const metadata = {
      ...(existingMetadata ?? buildSessionMetadata(session, null)),
      reviewActionState: payload,
    };

    await sessionAPI.saveSessionMetadata(
      metadata,
      session.workspacePath,
      session.remoteConnectionId,
      session.remoteSshHost
    );
  } catch (error) {
    log.warn('Failed to persist review action state', { sessionId: state.childSessionId, error });
    throw error;
  }
}

export async function clearPersistedReviewState(sessionId: string, workspacePath: string): Promise<void> {
  try {
    const existingMetadata = await sessionAPI.loadSessionMetadata(sessionId, workspacePath);
    if (!existingMetadata) return;

    const metadata = { ...existingMetadata };
    delete metadata.reviewActionState;

    await sessionAPI.saveSessionMetadata(
      metadata,
      workspacePath
    );
  } catch (error) {
    log.warn('Failed to clear persisted review action state', { sessionId, error });
  }
}

export async function loadPersistedReviewState(
  sessionId: string,
  workspacePath: string,
  remoteConnectionId?: string,
  remoteSshHost?: string
): Promise<ReviewActionPersistedState | null> {
  try {
    const metadata = await sessionAPI.loadSessionMetadata(
      sessionId,
      workspacePath,
      remoteConnectionId,
      remoteSshHost
    );
    return metadata?.reviewActionState ?? null;
  } catch (error) {
    log.warn('Failed to load persisted review action state', { sessionId, error });
    return null;
  }
}
