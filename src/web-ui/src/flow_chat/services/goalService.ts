import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { notificationService } from '@/shared/notification-system';
import type { Session } from '../types/flow-chat';
import { flowChatStore } from '../store/FlowChatStore';
import { FlowChatManager } from './FlowChatManager';

export { isGoalSlashCommand, parseGoalCommand } from './goalCommandParser';

export interface GoalCommandParams {
  session: Session;
  userHint?: string;
  loadingMessage: string;
  failedTitle: string;
  unknownErrorMessage: string;
  aiFailedMessage: string;
  activatedTitle: string;
}

export interface GoalCommandResult {
  goalText: string;
  successCriteria: string[];
}

export async function runGoalCommand(params: GoalCommandParams): Promise<GoalCommandResult> {
  if (!params.session.workspacePath) {
    throw new Error('A workspace is required to activate goal mode.');
  }

  const pendingId = `pending-${params.session.sessionId}-${Date.now()}`;
  const pendingTurn = flowChatStore.addLocalGoalPendingTurn({
    sessionId: params.session.sessionId,
    message: params.loadingMessage,
    pendingId,
  });
  let finalizedPendingTurn = false;

  try {
    const activation = await agentAPI.activateSessionGoal({
      sessionId: params.session.sessionId,
      userHint: params.userHint,
      workspacePath: params.session.workspacePath,
      remoteConnectionId: params.session.remoteConnectionId,
      remoteSshHost: params.session.remoteSshHost,
    });

    if (pendingTurn) {
      flowChatStore.deleteDialogTurn(params.session.sessionId, pendingTurn.id);
    }
    finalizedPendingTurn = true;

    const flowChatManager = FlowChatManager.getInstance();
    await flowChatManager.sendMessage(
      activation.kickoffMessage,
      params.session.sessionId,
      activation.displayMessage,
      undefined,
      undefined,
      {
        userMessageMetadata: {
          goalModeKickoff: true,
          goalModeCommand: params.userHint ? `/goal ${params.userHint}` : '/goal',
          goalText: activation.goalText,
          successCriteria: activation.successCriteria,
        },
      }
    );

    notificationService.success(activation.goalText, {
      title: params.activatedTitle,
      duration: 6000,
    });

    return {
      goalText: activation.goalText,
      successCriteria: activation.successCriteria,
    };
  } catch (error) {
    if (pendingTurn && !finalizedPendingTurn) {
      flowChatStore.deleteDialogTurn(params.session.sessionId, pendingTurn.id);
    }
    throw error;
  }
}

function resolveGoalCommandError(error: unknown, params: GoalCommandParams): string {
  if (!(error instanceof Error)) {
    return params.unknownErrorMessage;
  }

  const message = error.message.trim();
  if (!message) {
    return params.unknownErrorMessage;
  }

  if (
    /Goal func agent|AI client factory|provider timeout|Failed to get goal func agent/i.test(
      message
    )
  ) {
    return params.aiFailedMessage;
  }

  return message;
}

export async function runGoalCommandSafely(
  params: GoalCommandParams
): Promise<GoalCommandResult | null> {
  try {
    return await runGoalCommand(params);
  } catch (error) {
    notificationService.error(resolveGoalCommandError(error, params), {
      title: params.failedTitle,
      duration: 5000,
    });
    return null;
  }
}
