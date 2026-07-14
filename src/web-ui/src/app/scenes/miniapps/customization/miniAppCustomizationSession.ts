import type { CreateSessionRequest } from '@/infrastructure/api/service-api/AgentAPI';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('MiniAppCustomizationSession');

export interface BuildMiniAppCustomizationSessionRequestInput {
  sessionId: string;
  sessionName: string;
  workspacePath: string;
}

export function buildMiniAppCustomizationSessionRequest(
  input: BuildMiniAppCustomizationSessionRequestInput,
): CreateSessionRequest {
  return {
    sessionId: input.sessionId,
    sessionName: input.sessionName,
    agentType: 'agentic',
    workspacePath: input.workspacePath,
    sessionKind: 'subagent',
    config: {
      enableTools: true,
      safeMode: true,
      autoCompact: true,
      enableContextCompression: true,
    },
  };
}

export function createMiniAppCustomizationSessionId(appId: string): string {
  return `miniapp-customize-${appId}-${Date.now()}`;
}

export async function launchMiniAppCustomizationSession(params: {
  appId: string;
  appName: string;
  workspacePath: string;
  sessionName: string;
  prompt: string;
  displayMessage: string;
}): Promise<{ sessionId: string }> {
  const [
    { agentAPI },
    { FlowChatManager },
    { flowChatStore },
  ] = await Promise.all([
    import('@/infrastructure/api/service-api/AgentAPI'),
    import('@/flow_chat/services/FlowChatManager'),
    import('@/flow_chat/store/FlowChatStore'),
  ]);
  const request = buildMiniAppCustomizationSessionRequest({
    sessionId: createMiniAppCustomizationSessionId(params.appId),
    sessionName: params.sessionName,
    workspacePath: params.workspacePath,
  });
  const created = await agentAPI.createSession(request);

  flowChatStore.addExternalSession(
    created.sessionId,
    created.sessionName,
    created.agentType,
    params.workspacePath,
    {
      sessionKind: 'miniapp',
      isTransient: true,
      agentBackedTransient: true,
    },
  );

  await FlowChatManager.getInstance().sendMessage(
    params.prompt,
    created.sessionId,
    params.displayMessage,
    'agentic',
    undefined,
    {
      userMessageMetadata: {
        surface: 'miniapp_customization',
        appId: params.appId,
      },
    },
  );

  return { sessionId: created.sessionId };
}

export function cleanupMiniAppCustomizationSession(sessionId: string | null | undefined): void {
  if (!sessionId) {
    return;
  }

  void Promise.all([
    import('@/flow_chat/services/FlowChatManager'),
    import('@/infrastructure/api/service-api/AgentAPI'),
  ]).then(([{ FlowChatManager }, { agentAPI }]) => {
    try {
      FlowChatManager.getInstance().discardLocalSession(sessionId);
    } catch (error) {
      log.warn('Failed to remove MiniApp customization session locally', { sessionId, error });
    }

    return agentAPI.cancelSession(sessionId);
  }).catch((error) => {
    log.warn('Failed to clean up MiniApp customization session', { sessionId, error });
  });
}
