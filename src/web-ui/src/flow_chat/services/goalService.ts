import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { notificationService } from '@/shared/notification-system';
import type { Session } from '../types/flow-chat';
import { FlowChatManager } from './FlowChatManager';

export { isGoalSlashCommand, parseGoalCommand } from './goalCommandParser';

export interface GoalCommandParams {
  session: Session;
  userHint?: string;
  failedTitle: string;
  unknownErrorMessage: string;
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

  const activation = await agentAPI.activateSessionGoal({
    sessionId: params.session.sessionId,
    userHint: params.userHint,
    workspacePath: params.session.workspacePath,
    remoteConnectionId: params.session.remoteConnectionId,
    remoteSshHost: params.session.remoteSshHost,
  });

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
}

export async function runGoalCommandSafely(
  params: GoalCommandParams
): Promise<GoalCommandResult | null> {
  try {
    return await runGoalCommand(params);
  } catch (error) {
    notificationService.error(
      error instanceof Error ? error.message : params.unknownErrorMessage,
      {
        title: params.failedTitle,
        duration: 5000,
      }
    );
    return null;
  }
}
