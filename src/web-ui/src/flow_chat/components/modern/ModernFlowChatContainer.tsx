/**
 * Modern FlowChat container.
 * Uses virtual scrolling with Zustand and syncs legacy store state.
 */

import React, { useEffect, useMemo, useCallback, useRef, useState } from 'react';
import { VirtualMessageList, VirtualMessageListRef } from './VirtualMessageList';
import { FlowChatHeader } from './FlowChatHeader';
import { WelcomePanel } from '../WelcomePanel';
import { FlowChatContext, FlowChatContextValue } from './FlowChatContext';
import { useVirtualItems, useActiveSession, useVisibleTurnInfo } from '../../store/modernFlowChatStore';
import type { VirtualItem } from '../../store/modernFlowChatStore';
import { flowChatStore } from '../../store/FlowChatStore';
import { startAutoSync } from '../../services/storeSync';
import { globalEventBus } from '../../../infrastructure/event-bus';
import { getElementText, copyTextToClipboard } from '../../../shared/utils/textSelection';
import type { FlowChatConfig, FlowToolItem, DialogTurn, ModelRound, FlowItem } from '../../types/flow-chat';
import { notificationService } from '../../../shared/notification-system';
import { agentAPI } from '@/infrastructure/api';
import { ideControl } from '@/shared/services/ide-control';
import type { LineRange } from '@/component-library';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import path from 'path-browserify';
import { createLogger } from '@/shared/utils/logger';
import './ModernFlowChatContainer.scss';

const log = createLogger('ModernFlowChatContainer');

type ExploreGroupVirtualItem = Extract<VirtualItem, { type: 'explore-group' }>;

interface ModernFlowChatContainerProps {
  className?: string;
  config?: Partial<FlowChatConfig>;

  // Callbacks compatible with the legacy version.
  onFileViewRequest?: (filePath: string, fileName: string, lineRange?: LineRange) => void;
  onTabOpen?: (tabInfo: any) => void;
  onOpenVisualization?: (type: string, data: any) => void;
  onSwitchToChatPanel?: () => void;
}

export const ModernFlowChatContainer: React.FC<ModernFlowChatContainerProps> = ({
  className = '',
  config,
  onFileViewRequest,
  onTabOpen,
  onOpenVisualization,
  onSwitchToChatPanel,
}) => {
  const virtualItems = useVirtualItems();
  const activeSession = useActiveSession();
  const visibleTurnInfo = useVisibleTurnInfo();
  const virtualListRef = useRef<VirtualMessageListRef>(null);
  const { workspacePath } = useWorkspaceContext();
  
  // Explore group collapse state (key: groupId, true = user-expanded).
  const [exploreGroupStates, setExploreGroupStates] = useState<Map<string, boolean>>(new Map());
  
  const handleExploreGroupToggle = useCallback((groupId: string) => {
    setExploreGroupStates(prev => {
      const next = new Map(prev);
      next.set(groupId, !prev.get(groupId));
      return next;
    });
  }, []);
  
  const handleExpandAllInTurn = useCallback((turnId: string) => {
    const groupIds = virtualItems
      .filter((item): item is ExploreGroupVirtualItem =>
        item.type === 'explore-group' && 
        item.turnId === turnId
      )
      .map(item => item.data.groupId);
    
    setExploreGroupStates(prev => {
      const next = new Map(prev);
      [...new Set(groupIds)].forEach(id => next.set(id, true));
      return next;
    });
  }, [virtualItems]);
  
  const handleCollapseGroup = useCallback((groupId: string) => {
    setExploreGroupStates(prev => {
      const next = new Map(prev);
      next.set(groupId, false);
      return next;
    });
  }, []);
  
  useEffect(() => {
    const unsubscribe = startAutoSync();
    return () => {
      unsubscribe();
    };
  }, []);
  
  useEffect(() => {
    const unlisten = agentAPI.onSessionTitleGenerated((event) => {
      flowChatStore.updateSessionTitle(
        event.sessionId,
        event.title,
        'generated'
      );
    });

    return () => {
      unlisten();
    };
  }, []);
  
  useEffect(() => {
    const unsubscribe = globalEventBus.on('flowchat:copy-dialog', ({ dialogTurn }) => {
      if (!dialogTurn) {
        log.warn('Copy failed: dialog element not provided');
        return;
      }

      const dialogElement = dialogTurn as HTMLElement;
      const fullText = getElementText(dialogElement);
      
      if (!fullText || fullText.trim().length === 0) {
        notificationService.warning('Dialog is empty, nothing to copy');
        return;
      }

      copyTextToClipboard(fullText).then(success => {
        if (!success) {
          notificationService.error('Copy failed. Please try again.');
        }
      });
    });
    
    return unsubscribe;
  }, []);

  const handleToolConfirm = useCallback(async (toolId: string, updatedInput?: any) => {
    try {
      const latestState = flowChatStore.getState();
      const dialogTurns = Array.from(latestState.sessions.values()).flatMap(session => 
        Object.values(session.dialogTurns) as DialogTurn[]
      );

      let toolItem: FlowToolItem | null = null;
      let turnId: string | null = null;

      for (const turn of dialogTurns) {
        for (const modelRound of Object.values(turn.modelRounds) as ModelRound[]) {
          const item = modelRound.items.find((item: FlowItem) => 
            item.type === 'tool' && item.id === toolId
          ) as FlowToolItem;
          
          if (item) {
            toolItem = item;
            turnId = turn.id;
            break;
          }
        }
        if (toolItem) break;
      }

      if (!toolItem || !turnId) {
        notificationService.error(`Tool confirmation failed: tool item ${toolId} not found in current session`);
        return;
      }

      const finalInput = updatedInput || toolItem.toolCall?.input;
      
      const activeSessionId = latestState.activeSessionId;
      if (activeSessionId) {
        flowChatStore.updateModelRoundItem(activeSessionId, turnId, toolId, {
          userConfirmed: true,
          status: 'confirmed',
          toolCall: {
            ...toolItem.toolCall,
            input: finalInput
          }
        } as any);
      }

      if (!activeSessionId) {
        throw new Error('No active session ID');
      }

      const { agentService } = await import('../../../shared/services/agent-service');
      await agentService.confirmToolExecution(
        activeSessionId,
        toolId,
        'confirm',
        finalInput
      );
    } catch (error) {
      log.error('Tool confirmation failed', error);
      notificationService.error(`Tool confirmation failed: ${error}`);
    }
  }, []);

  const handleToolReject = useCallback(async (toolId: string) => {
    try {
      const latestState = flowChatStore.getState();
      const dialogTurns = Array.from(latestState.sessions.values()).flatMap(session => 
        Object.values(session.dialogTurns) as DialogTurn[]
      );

      let toolItem: FlowToolItem | null = null;
      let turnId: string | null = null;

      for (const turn of dialogTurns) {
        for (const modelRound of Object.values(turn.modelRounds) as ModelRound[]) {
          const item = modelRound.items.find((item: FlowItem) => 
            item.type === 'tool' && item.id === toolId
          ) as FlowToolItem;
          
          if (item) {
            toolItem = item;
            turnId = turn.id;
            break;
          }
        }
        if (toolItem) break;
      }

      if (!toolItem || !turnId) {
        log.warn('Tool rejection failed: tool item not found', { toolId });
        return;
      }

      const activeSessionId = latestState.activeSessionId;
      if (activeSessionId) {
        flowChatStore.updateModelRoundItem(activeSessionId, turnId, toolId, {
          userConfirmed: false,
          status: 'rejected'
        } as any);
      }

      if (!activeSessionId) {
        throw new Error('No active session ID');
      }

      const { agentService } = await import('../../../shared/services/agent-service');
      await agentService.confirmToolExecution(
        activeSessionId,
        toolId,
        'reject'
      );
    } catch (error) {
      log.error('Tool rejection failed', error);
      notificationService.error(`Tool rejection failed: ${error}`);
    }
  }, []);

  const handleFileViewRequest = useCallback(async (
    filePath: string,
    fileName: string,
    lineRange?: LineRange
  ) => {
    log.debug('File view request', {
      filePath,
      fileName,
      hasLineRange: !!lineRange,
      hasExternalCallback: !!onFileViewRequest
    });

    if (onFileViewRequest) {
      onFileViewRequest(filePath, fileName, lineRange);
      return;
    }

    let absoluteFilePath = filePath;

    const isWindowsAbsolutePath = /^[A-Za-z]:[\\/]/.test(filePath);

    if (!isWindowsAbsolutePath && !path.isAbsolute(filePath) && workspacePath) {
      absoluteFilePath = path.join(workspacePath, filePath);
      log.debug('Converted relative path to absolute', {
        relative: filePath,
        absolute: absoluteFilePath
      });
    }

    try {
      if (lineRange) {
        await ideControl.navigation.goToRange(
          absoluteFilePath,
          lineRange
        );
      } else {
        await ideControl.navigation.goToFile(absoluteFilePath);
      }
    } catch (error) {
      log.error('File navigation failed', error);
      notificationService.error(`Unable to open file: ${absoluteFilePath}`);
    }
  }, [onFileViewRequest, workspacePath]);

  const contextValue: FlowChatContextValue = useMemo(() => ({
    onFileViewRequest: handleFileViewRequest,
    onTabOpen,
    onOpenVisualization,
    onSwitchToChatPanel,
    onToolConfirm: handleToolConfirm,
    onToolReject: handleToolReject,
    sessionId: activeSession?.sessionId,
    config: {
      enableMarkdown: true,
      autoScroll: true,
      showTimestamps: false,
      maxHistoryRounds: 50,
      enableVirtualScroll: true,
      theme: 'dark',
      ...config,
    },
    exploreGroupStates,
    onExploreGroupToggle: handleExploreGroupToggle,
    onExpandAllInTurn: handleExpandAllInTurn,
    onCollapseGroup: handleCollapseGroup,
  }), [
    handleFileViewRequest,
    onTabOpen,
    onOpenVisualization,
    onSwitchToChatPanel,
    handleToolConfirm,
    handleToolReject,
    activeSession?.sessionId,
    config,
    exploreGroupStates,
    handleExploreGroupToggle,
    handleExpandAllInTurn,
    handleCollapseGroup,
  ]);
  
  const handlePreviousTurn = useCallback(() => {
    if (virtualListRef.current && visibleTurnInfo && visibleTurnInfo.turnIndex > 1) {
      virtualListRef.current.scrollToTurn(visibleTurnInfo.turnIndex - 1);
    }
  }, [visibleTurnInfo]);

  const handleNextTurn = useCallback(() => {
    if (virtualListRef.current && visibleTurnInfo && visibleTurnInfo.turnIndex < visibleTurnInfo.totalTurns) {
      virtualListRef.current.scrollToTurn(visibleTurnInfo.turnIndex + 1);
    }
  }, [visibleTurnInfo]);
  
  return (
    <FlowChatContext.Provider value={contextValue}>
      <div className={`modern-flowchat-container ${className}`}>
        <FlowChatHeader
          currentTurnIndex={visibleTurnInfo?.turnIndex ?? 0}
          totalTurns={visibleTurnInfo?.totalTurns ?? 0}
          currentUserMessage={visibleTurnInfo?.userMessage ?? ''}
          visible={virtualItems.length > 0}
          onPreviousTurn={handlePreviousTurn}
          onNextTurn={handleNextTurn}
          sessionId={activeSession?.sessionId}
        />

        <div className="modern-flowchat-container__messages">
          {virtualItems.length === 0 ? (
            <WelcomePanel
              key={activeSession?.sessionId ?? 'welcome'}
              sessionMode={activeSession?.mode}
              onQuickAction={(command) => {
                window.dispatchEvent(new CustomEvent('fill-chat-input', {
                  detail: { message: command }
                }));
              }}
            />
          ) : (
            <VirtualMessageList ref={virtualListRef} />
          )}
        </div>
      </div>
    </FlowChatContext.Provider>
  );
};

ModernFlowChatContainer.displayName = 'ModernFlowChatContainer';
