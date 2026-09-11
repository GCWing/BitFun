import { useMemo, useSyncExternalStore } from 'react';
import { flowChatStore } from '../../store/FlowChatStore';
import type { FlowChatState, Session } from '../../types/flow-chat';
import { findReviewTaskOutcome, type ReviewTaskOutcome } from '../../utils/reviewTaskOutcome';

type ParentMetadata = Pick<Session, 'title' | 'workspacePath' | 'remoteConnectionId' | 'remoteSshHost'>;

interface BtwSessionState {
  childSession: Session | undefined;
  parentMetadata: ParentMetadata | undefined;
  reviewTaskOutcome: ReviewTaskOutcome | null;
}

/** Subscribe to this pane's sessions without retaining the global store snapshot. */
export function useBtwSessionState(
  childSessionId?: string,
  parentSessionId?: string,
  reviewCheck = false,
): BtwSessionState {
  const source = useMemo(() => {
    let snapshot: BtwSessionState = {
      childSession: undefined, parentMetadata: undefined, reviewTaskOutcome: null,
    };
    let reviewTurns: Session['dialogTurns'] | undefined;
    let reviewToolCallId: string | undefined;
    let reviewTaskOutcome: ReviewTaskOutcome | null = null;
    const select = (state: FlowChatState) => {
      const childSession = childSessionId ? state.sessions.get(childSessionId) : undefined;
      const parentSession = parentSessionId ? state.sessions.get(parentSessionId) : undefined;
      let parentMetadata = snapshot.parentMetadata;
      if (
        Boolean(parentMetadata) !== Boolean(parentSession) ||
        parentMetadata?.title !== parentSession?.title ||
        parentMetadata?.workspacePath !== parentSession?.workspacePath ||
        parentMetadata?.remoteConnectionId !== parentSession?.remoteConnectionId ||
        parentMetadata?.remoteSshHost !== parentSession?.remoteSshHost
      ) {
        parentMetadata = parentSession ? {
          title: parentSession.title,
          workspacePath: parentSession.workspacePath,
          remoteConnectionId: parentSession.remoteConnectionId,
          remoteSshHost: parentSession.remoteSshHost,
        } : undefined;
      }
      // Ordinary child panes never scan or retain the parent's transcript.
      // Review panes publish only the linked Task's semantic outcome.
      if (reviewCheck && (
        reviewTurns !== parentSession?.dialogTurns ||
        reviewToolCallId !== childSession?.parentToolCallId
      )) {
        reviewTurns = parentSession?.dialogTurns;
        reviewToolCallId = childSession?.parentToolCallId;
        reviewTaskOutcome = findReviewTaskOutcome(parentSession, reviewToolCallId);
      }
      if (snapshot.childSession !== childSession || snapshot.parentMetadata !== parentMetadata ||
        snapshot.reviewTaskOutcome !== reviewTaskOutcome) {
        snapshot = { childSession, parentMetadata, reviewTaskOutcome };
      }
      return snapshot;
    };
    return {
      getSnapshot: () => select(flowChatStore.getState()),
      subscribe: (notify: () => void) => flowChatStore.subscribeSelector(select, notify),
    };
  }, [childSessionId, parentSessionId, reviewCheck]);

  // React rechecks after subscribing, covering updates between render and commit
  // and immediately selecting the new pair when a pane changes sessions.
  return useSyncExternalStore(source.subscribe, source.getSnapshot, source.getSnapshot);
}
