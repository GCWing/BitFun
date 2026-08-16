import type { Session } from '../types/flow-chat';
import { sessionWorktreeBindingSubscriptionKey } from './sessionWorktree';

/**
 * Render-relevant Session facts consumed directly by ChatInput.
 *
 * Keep this key in sync with the component's render reads. In particular, a
 * Harness update must invalidate the snapshot even though the Session id and
 * ordinary mode stay unchanged.
 */
export function chatInputSessionSubscriptionKey(session: Session): string {
  return (
    `${session.sessionId}|${session.mode ?? ''}|${session.title ?? ''}|${session.workspacePath ?? ''}|` +
    `${session.remoteConnectionId ?? ''}|${session.remoteSshHost ?? ''}|${session.lastSubmittedMode ?? ''}|` +
    `${session.config.executionProfile?.harnessProfileId ?? ''}|` +
    `${session.currentAcpContextUsage?.used ?? ''}|${session.currentAcpContextUsage?.size ?? ''}|` +
    `${session.currentTokenUsage?.inputTokens ?? ''}|${session.maxContextTokens ?? ''}|` +
    `${session.needsUserAttention ? '1' : '0'}|${session.dialogTurns.length}|` +
    `${session.totalTurnCount ?? ''}|${session.turnCatalog?.totalTurnCount ?? ''}|` +
    `${JSON.stringify(session.config.dispatchTarget ?? null)}|` +
    `${session.config.dispatchApprovalPolicy ?? ''}|${session.config.dispatchJobState ?? ''}|` +
    `${sessionWorktreeBindingSubscriptionKey(session)}`
  );
}
