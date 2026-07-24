/**
 * Tool confirmation/rejection actions for Modern FlowChat.
 */

import { useCallback } from 'react';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import {
  ACPClientAPI,
} from '@/infrastructure/api/service-api/ACPClientAPI';
import { flowChatStore } from '../../store/FlowChatStore';
import type { DialogTurn, FlowItem, FlowToolItem, ModelRound, ToolRejectOptions } from '../../types/flow-chat';

const log = createLogger('useFlowChatToolActions');

interface ResolvedToolContext {
  sessionId: string | null;
  toolItem: FlowToolItem | null;
  turnId: string | null;
}

function resolveToolContext(toolId: string): ResolvedToolContext {
  const latestState = flowChatStore.getState();
  let sessionId: string | null = null;
  let toolItem: FlowToolItem | null = null;
  let turnId: string | null = null;

  for (const [candidateSessionId, session] of latestState.sessions) {
    for (const turn of session.dialogTurns as DialogTurn[]) {
      for (const modelRound of turn.modelRounds as ModelRound[]) {
        const item = modelRound.items.find((candidate: FlowItem) => (
          candidate.type === 'tool' && candidate.id === toolId
        )) as FlowToolItem | undefined;

        if (item) {
          sessionId = candidateSessionId;
          toolItem = item;
          turnId = turn.id;
          break;
        }
      }

      if (toolItem) {
        break;
      }
    }

    if (toolItem) break;
  }

  return {
    sessionId,
    toolItem,
    turnId,
  };
}

export function useFlowChatToolActions() {
  const handleToolConfirm = useCallback(async (
    toolId: string,
    permissionOptionId?: string,
    approve = true,
  ) => {
    try {
      const { sessionId, toolItem, turnId } = resolveToolContext(toolId);

      if (!sessionId || !toolItem || !turnId) {
        notificationService.error(`Tool confirmation failed: tool item ${toolId} not found in current session`);
        return;
      }

      flowChatStore.updateModelRoundItem(sessionId, turnId, toolId, {
        userConfirmed: approve,
        status: approve ? 'confirmed' : 'rejected',
        ...(approve ? {} : {
          requiresConfirmation: false,
          acpPermission: undefined,
          isParamsStreaming: false,
          toolResult: {
            result: null,
            success: false,
            error: 'User rejected operation',
          },
          endTime: Date.now(),
        }),
      } as any);

      const acpPermission = toolItem.acpPermission;
      if (acpPermission?.permissionId) {
        await ACPClientAPI.submitPermissionResponse({
          permissionId: acpPermission.permissionId,
          approve,
          optionId: permissionOptionId,
        });
        return;
      }

      log.warn('Ignoring legacy BitFun tool confirmation without a permission request id', { toolId });
    } catch (error) {
      log.error('Tool confirmation failed', error);
      notificationService.error(`Tool confirmation failed: ${error}`);
    }
  }, []);

  const handleToolReject = useCallback(async (toolId: string, options?: ToolRejectOptions) => {
    try {
      const { sessionId, toolItem, turnId } = resolveToolContext(toolId);

      if (!sessionId || !toolItem || !turnId) {
        log.warn('Tool rejection failed: tool item not found', { toolId });
        return;
      }

      flowChatStore.updateModelRoundItem(sessionId, turnId, toolId, {
        userConfirmed: false,
        status: 'rejected',
        requiresConfirmation: false,
        acpPermission: undefined,
        isParamsStreaming: false,
        toolResult: {
          result: null,
          success: false,
          error: 'User rejected operation',
        },
        endTime: Date.now(),
      } as any);

      const acpPermission = toolItem.acpPermission;
      if (acpPermission?.permissionId) {
        await ACPClientAPI.submitPermissionResponse({
          permissionId: acpPermission.permissionId,
          approve: false,
          optionId: options?.permissionOptionId,
        });
        return;
      }

      log.warn('Ignoring legacy BitFun tool rejection without a permission request id', { toolId });
    } catch (error) {
      log.error('Tool rejection failed', error);
      notificationService.error(`Tool rejection failed: ${error}`);
    }
  }, []);

  return {
    handleToolConfirm,
    handleToolReject,
  };
}
