import type { Session } from '../types/flow-chat';

export interface InterruptedTurnRecoveryCandidate {
  sessionId: string;
  turnId: string;
  executionGeneration: number;
}

export interface InterruptedTurnRecoveryComposerState {
  draft: string;
  hasComposerAttachments: boolean;
  executionIdle: boolean;
  desktopRuntime: boolean;
  peerMode: boolean;
  acpSession: boolean;
  modeChangePending: boolean;
  modelChangePending: boolean;
}

/**
 * Pure capability predicate for the input button. Runtime validation remains
 * authoritative; this function prevents unsupported or ambiguous states from
 * presenting a recovery action in the first place.
 */
export function selectInterruptedTurnRecovery(
  session: Session | undefined,
  composer: InterruptedTurnRecoveryComposerState,
): InterruptedTurnRecoveryCandidate | null {
  if (
    !session
    || session.sessionKind !== 'normal'
    || !composer.executionIdle
    || !composer.desktopRuntime
    || composer.peerMode
    || composer.acpSession
    || composer.modeChangePending
    || composer.modelChangePending
    || composer.draft.trim().length > 0
    || composer.hasComposerAttachments
    || session.remoteConnectionId
    || session.remoteSshHost
    || session.config.remoteConnectionId
    || session.config.remoteSshHost
    || session.config.dispatchTarget
    || session.config.dispatchJobId
  ) {
    return null;
  }

  const goalStatus = session.threadGoal?.status.toLowerCase();
  if (goalStatus === 'active' || goalStatus === 'paused') {
    return null;
  }

  const turn = session.dialogTurns.at(-1);
  if (
    !turn
    || turn.status !== 'cancelled'
    || turn.finishReason !== 'interrupted'
    || turn.recovery?.status !== 'interrupted'
    || !Number.isSafeInteger(turn.recovery.executionGeneration)
    || turn.recovery.executionGeneration < 0
  ) {
    return null;
  }

  const selectedMode = session.mode || session.config.agentType;
  if (turn.agentType && selectedMode && turn.agentType !== selectedMode) {
    return null;
  }
  if (
    turn.recovery.modelId
    && session.config.modelName
    && turn.recovery.modelId !== session.config.modelName
  ) {
    return null;
  }

  return {
    sessionId: session.sessionId,
    turnId: turn.id,
    executionGeneration: turn.recovery.executionGeneration,
  };
}

export function hasInterruptedTurnHoldingQueue(session: Session | undefined): boolean {
  const latest = session?.dialogTurns.at(-1);
  return Boolean(
    latest
    && latest.status === 'cancelled'
    && latest.finishReason === 'interrupted'
    && latest.recovery?.status === 'interrupted',
  );
}
