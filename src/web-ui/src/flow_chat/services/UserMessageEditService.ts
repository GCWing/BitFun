import { snapshotAPI } from '@/infrastructure/api';
import { globalEventBus } from '@/infrastructure/event-bus';
import { createLogger } from '@/shared/utils/logger';
import { flowChatStore } from '../store/FlowChatStore';
import { pendingQueueManager } from './flow-chat-manager/PendingQueueModule';
import { stateMachineManager } from '../state-machine';
import { SessionExecutionEvent, SessionExecutionState } from '../state-machine/types';

const log = createLogger('UserMessageEditService');

export interface UserMessageEditImpact {
  willStopRunningTask: boolean;
  willRestoreFiles: boolean;
  willDeleteTurns: boolean;
  willRerun: boolean;
}

export interface EditAndRerunUserMessageRequest {
  sessionId: string;
  turnId: string;
  turnIndex: number;
  originalContent: string;
  editedContent: string;
  agentType?: string;
  rerun: (content: string, agentType?: string) => Promise<void>;
}

export function describeUserMessageEditImpact(sessionId: string): UserMessageEditImpact {
  const currentState = stateMachineManager.getCurrentState(sessionId);
  return {
    willStopRunningTask:
      currentState === SessionExecutionState.PROCESSING ||
      currentState === SessionExecutionState.FINISHING,
    willRestoreFiles: true,
    willDeleteTurns: true,
    willRerun: true,
  };
}

export function canEditUserMessage(request: {
  sessionId?: string | null;
  turnIndex: number;
  hasImages?: boolean;
  isUsageReportMessage?: boolean;
  steeringStatus?: string;
  isRemoteSession?: boolean;
  isSubmitting?: boolean;
}): boolean {
  return Boolean(
    request.sessionId &&
      request.turnIndex >= 0 &&
      !request.hasImages &&
      !request.isUsageReportMessage &&
      !request.steeringStatus &&
      !request.isRemoteSession &&
      !request.isSubmitting,
  );
}

export async function editAndRerunUserMessage(
  request: EditAndRerunUserMessageRequest,
): Promise<void> {
  const trimmedEditedContent = request.editedContent.trim();
  if (!trimmedEditedContent) {
    throw new Error('Edited message cannot be empty');
  }

  if (trimmedEditedContent === request.originalContent.trim()) {
    return;
  }

  const session = flowChatStore.getState().sessions.get(request.sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${request.sessionId}`);
  }

  const targetTurn = session.dialogTurns[request.turnIndex];
  if (!targetTurn || targetTurn.id !== request.turnId) {
    throw new Error('Message edit target is no longer available');
  }

  const triggerSource = targetTurn.userMessage.metadata?.triggerSource;
  if (triggerSource && triggerSource !== 'desktop_ui') {
    throw new Error('Only messages sent from the desktop chat input can be edited');
  }

  if (pendingQueueManager.list(request.sessionId).length > 0) {
    throw new Error('Edit the message after clearing the pending queue');
  }

  const currentState = stateMachineManager.getCurrentState(request.sessionId);
  if (currentState === SessionExecutionState.PROCESSING || currentState === SessionExecutionState.FINISHING) {
    await stateMachineManager.transition(request.sessionId, SessionExecutionEvent.USER_CANCEL);
  } else if (currentState === SessionExecutionState.ERROR) {
    await stateMachineManager.transition(request.sessionId, SessionExecutionEvent.RESET);
  }

  log.info('Editing user message and rerunning from turn', {
    sessionId: request.sessionId,
    turnIndex: request.turnIndex,
    turnId: request.turnId,
  });

  const restoredFiles = await snapshotAPI.rollbackToTurn(request.sessionId, request.turnIndex, true);

  flowChatStore.truncateDialogTurnsFrom(request.sessionId, request.turnIndex);

  globalEventBus.emit('file-tree:refresh');
  restoredFiles.forEach(filePath => {
    globalEventBus.emit('editor:file-changed', { filePath });
  });

  await request.rerun(trimmedEditedContent, request.agentType);
}
