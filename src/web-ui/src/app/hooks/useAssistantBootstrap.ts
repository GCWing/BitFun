import { useCallback, useEffect, useRef } from 'react';
import {
  agentAPI,
  type EnsureAssistantBootstrapResponse,
} from '@/infrastructure/api/service-api/AgentAPI';
import { useI18n } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { WorkspaceKind, type WorkspaceInfo } from '@/shared/types';

const log = createLogger('AssistantBootstrap');

interface BootstrapRequest {
  workspacePath: string;
  sessionId: string;
}

interface ActiveBootstrapAttempt extends BootstrapRequest {
  turnId: string;
}

export function useAssistantBootstrap() {
  const { t } = useI18n('notifications');
  const activeAttemptRef = useRef<ActiveBootstrapAttempt | null>(null);
  const pendingRequestRef = useRef<BootstrapRequest | null>(null);
  const latestWorkspacePathRef = useRef<string | null>(null);
  const inFlightWorkspacePathRef = useRef<string | null>(null);
  const blockedNoticeShownRef = useRef<Set<string>>(new Set());
  const requestBootstrapRef = useRef<(request: BootstrapRequest) => void>(() => {});

  const drainPendingRequest = useCallback(() => {
    const pending = pendingRequestRef.current;
    if (!pending) {
      return;
    }

    if (latestWorkspacePathRef.current !== pending.workspacePath) {
      pendingRequestRef.current = null;
      return;
    }

    pendingRequestRef.current = null;
    requestBootstrapRef.current(pending);
  }, []);

  const clearActiveAttempt = useCallback(
    (event: { sessionId?: string; turnId?: string }) => {
      const activeAttempt = activeAttemptRef.current;
      if (!activeAttempt) {
        return;
      }

      if (event.sessionId !== activeAttempt.sessionId || event.turnId !== activeAttempt.turnId) {
        return;
      }

      activeAttemptRef.current = null;
      drainPendingRequest();
    },
    [drainPendingRequest]
  );

  useEffect(() => {
    const unlistenCompleted = agentAPI.onDialogTurnCompleted(clearActiveAttempt);
    const unlistenFailed = agentAPI.onDialogTurnFailed(clearActiveAttempt);
    const unlistenCancelled = agentAPI.onDialogTurnCancelled(clearActiveAttempt);

    return () => {
      unlistenCompleted();
      unlistenFailed();
      unlistenCancelled();
    };
  }, [clearActiveAttempt]);

  const handleEnsureResponse = useCallback(
    (request: BootstrapRequest, response: EnsureAssistantBootstrapResponse): void => {
      switch (response.status) {
        case 'started':
          if (!response.turnId) {
            log.warn('Assistant bootstrap started without turnId', { request, response });
            return;
          }
          activeAttemptRef.current = {
            ...request,
            sessionId: response.sessionId,
            turnId: response.turnId,
          };
          blockedNoticeShownRef.current.delete(request.workspacePath);
          log.info('Assistant bootstrap started', {
            workspacePath: request.workspacePath,
            sessionId: response.sessionId,
            turnId: response.turnId,
          });
          return;
        case 'blocked':
          if (
            response.reason === 'model_unavailable' &&
            !blockedNoticeShownRef.current.has(request.workspacePath)
          ) {
            blockedNoticeShownRef.current.add(request.workspacePath);
            notificationService.info(t('info.assistantBootstrapWaitingForModelConfiguration'), {
              duration: 5000,
            });
          }
          log.info('Assistant bootstrap blocked', {
            workspacePath: request.workspacePath,
            sessionId: request.sessionId,
            reason: response.reason,
            detail: response.detail,
          });
          return;
        case 'skipped':
          if (response.reason === 'bootstrap_not_required') {
            blockedNoticeShownRef.current.delete(request.workspacePath);
          }
          log.debug('Assistant bootstrap skipped', {
            workspacePath: request.workspacePath,
            sessionId: request.sessionId,
            reason: response.reason,
          });
          return;
        default:
          return;
      }
    },
    [t]
  );

  const requestBootstrap = useCallback(
    async (request: BootstrapRequest): Promise<void> => {
      const activeAttempt = activeAttemptRef.current;
      if (activeAttempt) {
        if (activeAttempt.workspacePath === request.workspacePath) {
          return;
        }
        pendingRequestRef.current = request;
        return;
      }

      const inFlightWorkspacePath = inFlightWorkspacePathRef.current;
      if (inFlightWorkspacePath) {
        if (inFlightWorkspacePath === request.workspacePath) {
          return;
        }
        pendingRequestRef.current = request;
        return;
      }

      inFlightWorkspacePathRef.current = request.workspacePath;

      try {
        const response = await agentAPI.ensureAssistantBootstrap({
          sessionId: request.sessionId,
          workspacePath: request.workspacePath,
        });
        handleEnsureResponse(request, response);
      } catch (error) {
        log.error('Failed to ensure assistant bootstrap', {
          workspacePath: request.workspacePath,
          sessionId: request.sessionId,
          error,
        });
      } finally {
        if (inFlightWorkspacePathRef.current === request.workspacePath) {
          inFlightWorkspacePathRef.current = null;
        }

        if (!activeAttemptRef.current) {
          drainPendingRequest();
        }
      }
    },
    [drainPendingRequest, handleEnsureResponse]
  );

  useEffect(() => {
    requestBootstrapRef.current = (request: BootstrapRequest) => {
      void requestBootstrap(request);
    };
  }, [requestBootstrap]);

  const ensureForWorkspace = useCallback(
    (workspace: WorkspaceInfo | null | undefined, sessionId?: string | null): void => {
      latestWorkspacePathRef.current = workspace?.rootPath ?? null;

      if (
        !workspace ||
        workspace.workspaceKind !== WorkspaceKind.Assistant ||
        !sessionId
      ) {
        pendingRequestRef.current = null;
        return;
      }

      void requestBootstrap({
        workspacePath: workspace.rootPath,
        sessionId,
      });
    },
    [requestBootstrap]
  );

  return {
    ensureForWorkspace,
  };
}
