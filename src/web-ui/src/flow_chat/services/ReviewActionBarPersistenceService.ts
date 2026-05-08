/**
 * Review Action Bar persistence service.
 *
 * Persists review action bar state to session metadata via the backend API,
 * aligning with the existing session persistence architecture.
 */

import { createLogger } from '@/shared/utils/logger';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { flowChatStore } from '../store/FlowChatStore';
import type { ReviewActionBarState } from '../store/deepReviewActionBarStore';
import type { ReviewActionPersistedState } from '@/shared/types/session-history';

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
    await sessionAPI.saveSessionMetadata(
      {
        sessionId: state.childSessionId,
        reviewActionState: payload,
      } as any,
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
    await sessionAPI.saveSessionMetadata(
      {
        sessionId,
        reviewActionState: undefined,
      } as any,
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
