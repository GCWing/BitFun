import type { DialogTurn, FlowToolItem, ModelRound, Session } from '../types/flow-chat';
import { flowChatStore } from '../store/FlowChatStore';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ReviewSessionMarkerService');

export type ReviewSessionMarkerKind = 'review' | 'deep_review';

export interface InsertReviewSessionMarkerParams {
  parentSessionId: string;
  childSessionId: string;
  kind: ReviewSessionMarkerKind;
  title: string;
  requestedFiles: string[];
  parentDialogTurnId?: string;
}

function findTargetTurn(session: Session, parentDialogTurnId?: string) {
  if (parentDialogTurnId) {
    const matched = session.dialogTurns.find(turn => turn.id === parentDialogTurnId);
    if (matched) {
      return matched;
    }
  }

  return session.dialogTurns[session.dialogTurns.length - 1];
}

function findTargetRound(turn?: DialogTurn): ModelRound | undefined {
  if (!turn) {
    return undefined;
  }

  return turn.modelRounds[turn.modelRounds.length - 1];
}

export function insertReviewSessionSummaryMarker(params: InsertReviewSessionMarkerParams): boolean {
  const parentSession = flowChatStore.getState().sessions.get(params.parentSessionId);
  const targetTurn = parentSession ? findTargetTurn(parentSession, params.parentDialogTurnId) : undefined;
  const targetRound = findTargetRound(targetTurn);

  if (!parentSession || !targetTurn || !targetRound) {
    log.warn('Unable to insert review session summary marker', {
      parentSessionId: params.parentSessionId,
      childSessionId: params.childSessionId,
      hasParentSession: Boolean(parentSession),
      hasTargetTurn: Boolean(targetTurn),
      hasTargetRound: Boolean(targetRound),
    });
    return false;
  }

  const markerId = `review_summary_${params.childSessionId}`;
  const markerItem: FlowToolItem = {
    id: markerId,
    type: 'tool',
    timestamp: Date.now(),
    status: 'completed',
    toolName: 'ReviewSessionSummary',
    toolCall: {
      id: markerId,
      input: {
        childSessionId: params.childSessionId,
        parentSessionId: params.parentSessionId,
        kind: params.kind,
        title: params.title,
        requestedFiles: params.requestedFiles,
      },
    },
    requiresConfirmation: false,
  };

  flowChatStore.addModelRoundItem(
    params.parentSessionId,
    targetTurn.id,
    markerItem,
    targetRound.id,
  );
  return true;
}
