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
import { useModernFlowChatStore } from '../../store/modernFlowChatStore';
import { globalEventBus } from '../../../infrastructure/event-bus';
import { getElementText, copyTextToClipboard } from '../../../shared/utils/textSelection';
import type { FlowChatConfig, FlowToolItem, DialogTurn, ModelRound, FlowItem } from '../../types/flow-chat';
import { notificationService } from '../../../shared/notification-system';
import { agentAPI } from '@/infrastructure/api';
import { fileTabManager } from '@/shared/services/FileTabManager';
import type { LineRange } from '@/component-library';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import path from 'path-browserify';
import { createLogger } from '@/shared/utils/logger';
import { flowChatManager } from '../../services/FlowChatManager';
import './ModernFlowChatContainer.scss';

const log = createLogger('ModernFlowChatContainer');

type ExploreGroupVirtualItem = Extract<VirtualItem, { type: 'explore-group' }>;

interface ModernFlowChatContainerProps {
  className?: string;
  config?: Partial<FlowChatConfig>;

  // Callbacks compatible with the legacy version.
  onFileViewRequest?: (filePath: string, fileName: string, lineRange?: LineRange) => void;
  onTabOpen?: (tabInfo: any, sessionId?: string, panelType?: string) => void;
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

  useEffect(() => {
    const unsubscribe = globalEventBus.on<{
      sessionId: string;
      turnIndex?: number;
      itemId?: string;
    }>('flowchat:focus-item', async ({ sessionId, turnIndex, itemId }) => {
      if (!sessionId) return;

      const waitFor = async (predicate: () => boolean, timeoutMs: number): Promise<boolean> => {
        const start = performance.now();
        while (performance.now() - start < timeoutMs) {
          if (predicate()) return true;
          await new Promise<void>(r => requestAnimationFrame(() => r()));
        }
        return predicate();
      };

      // Switch session first (if needed). Note: Modern virtual list methods are bound to
      // the current render; after switching sessions we must wait a frame or two so
      // VirtualMessageList updates its imperative ref and item map before scrolling.
      if (activeSession?.sessionId !== sessionId) {
        try {
          await flowChatManager.switchChatSession(sessionId);
        } catch (e) {
          log.warn('Failed to switch session for focus request', { sessionId, e });
          return;
        }
      }

      // Wait until the modern store and list ref have caught up to the target session.
      // This avoids a common race where scrollToTurn no-ops against the previous session's item list.
      await waitFor(() => {
        const modernActive = useModernFlowChatStore.getState().activeSession?.sessionId;
        return modernActive === sessionId && !!virtualListRef.current;
      }, 1500);

      let resolvedVirtualIndex: number | undefined = undefined;
      let resolvedTurnIndex = turnIndex;
      if (itemId) {
        const s = flowChatStore.getState().sessions.get(sessionId);
        if (s) {
          for (let i = 0; i < s.dialogTurns.length; i++) {
            const t = s.dialogTurns[i];
            const found = t.modelRounds?.some(r => r.items?.some(it => it.id === itemId));
            if (found) {
              resolvedTurnIndex = i + 1;
              break;
            }
          }
        }

        // Prefer a precise virtual scroll target so the marker is actually rendered.
        // Scrolling to the turn's user message can leave the marker far outside the rendered range.
        const currentVirtualItems = useModernFlowChatStore.getState().virtualItems;
        for (let i = 0; i < currentVirtualItems.length; i++) {
          const vi = currentVirtualItems[i];
          if (vi.type === 'model-round') {
            const hit = vi.data?.items?.some((it: any) => it?.id === itemId);
            if (hit) {
              resolvedVirtualIndex = i;
              break;
            }
          } else if (vi.type === 'explore-group') {
            const hit = vi.data?.allItems?.some((it: any) => it?.id === itemId);
            if (hit) {
              resolvedVirtualIndex = i;
              break;
            }
          }
        }
      }

      // Scroll to the most precise target we have.
      if (resolvedVirtualIndex != null && virtualListRef.current) {
        virtualListRef.current.scrollToIndex(resolvedVirtualIndex);
      } else if (resolvedTurnIndex && virtualListRef.current) {
        virtualListRef.current.scrollToTurn(resolvedTurnIndex);
      }

      if (!itemId) return;

      // Wait two frames for Virtuoso to settle after instant scrollToIndex before
      // searching the DOM. This avoids finding an element that Virtuoso is about
      // to recycle when it processes the new scroll position.
      await new Promise<void>(r => requestAnimationFrame(() => requestAnimationFrame(() => r())));

      // Then focus the specific flow item (marker) within the DOM.
      // Retry a few times because virtualization/paint can lag behind the scroll.
      const maxAttempts = 120;
      let attempts = 0;
      const tryFocus = () => {
        attempts++;
        const el = document.querySelector(`[data-flow-item-id="${CSS.escape(itemId)}"]`) as HTMLElement | null;
        if (!el) {
          // Keep nudging the list to the right neighborhood; scrollToTurn can be preempted by
          // stick-to-bottom mode during session switches or streaming updates.
          if (attempts % 12 === 0 && virtualListRef.current) {
            if (resolvedVirtualIndex != null) {
              virtualListRef.current.scrollToIndex(resolvedVirtualIndex);
            } else if (resolvedTurnIndex) {
              virtualListRef.current.scrollToTurn(resolvedTurnIndex);
            }
          }
          if (attempts < maxAttempts) {
            requestAnimationFrame(tryFocus);
          }
          return;
        }

        el.classList.add('flowchat-flow-item--focused');
        window.setTimeout(() => el.classList.remove('flowchat-flow-item--focused'), 1600);
      };

      requestAnimationFrame(tryFocus);
    });

    return unsubscribe;
  }, [activeSession?.sessionId]);

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
      fileTabManager.openFile({
        filePath: absoluteFilePath,
        fileName,
        workspacePath,
        jumpToRange: lineRange,
        mode: 'agent',
      });
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
  
  return (
    <FlowChatContext.Provider value={contextValue}>
      <div className={`modern-flowchat-container ${className}`}>
        <FlowChatHeader
          currentTurnIndex={visibleTurnInfo?.turnIndex ?? 0}
          totalTurns={visibleTurnInfo?.totalTurns ?? 0}
          currentUserMessage={visibleTurnInfo?.userMessage ?? ''}
          visible={virtualItems.length > 0}
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
