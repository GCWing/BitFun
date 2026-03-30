/**
 * Modern FlowChat container.
 * Uses virtual scrolling with Zustand and syncs legacy store state.
 */

import React, { useMemo, useCallback, useRef, useEffect, useState } from 'react';
import { VirtualMessageList, VirtualMessageListRef } from './VirtualMessageList';
import { FlowChatHeader, type FlowChatHeaderTurnSummary } from './FlowChatHeader';
import { WelcomePanel } from '../WelcomePanel';
import { FlowChatContext, FlowChatContextValue } from './FlowChatContext';
import { useExploreGroupState } from './useExploreGroupState';
import { useFlowChatFileActions } from './useFlowChatFileActions';
import { useFlowChatNavigation } from './useFlowChatNavigation';
import { useFlowChatCopyDialog } from './useFlowChatCopyDialog';
import { useFlowChatSessionRelationship } from './useFlowChatSessionRelationship';
import { useFlowChatSync } from './useFlowChatSync';
import { useFlowChatToolActions } from './useFlowChatToolActions';
import { useVirtualItems, useActiveSession, useVisibleTurnInfo, type VisibleTurnInfo } from '../../store/modernFlowChatStore';
import type { FlowChatConfig } from '../../types/flow-chat';
import type { LineRange } from '@/component-library';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import './ModernFlowChatContainer.scss';

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
  const [pendingHeaderTurnId, setPendingHeaderTurnId] = useState<string | null>(null);
  const autoPinnedSessionIdRef = useRef<string | null>(null);
  const virtualListRef = useRef<VirtualMessageListRef>(null);
  const { workspacePath } = useWorkspaceContext();
  const { btwOrigin, btwParentTitle } = useFlowChatSessionRelationship(activeSession);
  const {
    exploreGroupStates,
    onExploreGroupToggle: handleExploreGroupToggle,
    onExpandGroup: handleExpandGroup,
    onExpandAllInTurn: handleExpandAllInTurn,
    onCollapseGroup: handleCollapseGroup,
  } = useExploreGroupState(virtualItems);
  const { handleToolConfirm, handleToolReject } = useFlowChatToolActions();
  const { handleFileViewRequest } = useFlowChatFileActions({
    workspacePath,
    onFileViewRequest,
  });

  useFlowChatSync();
  useFlowChatCopyDialog();

  useFlowChatNavigation({
    activeSessionId: activeSession?.sessionId,
    virtualItems,
    virtualListRef,
  });

  const contextValue: FlowChatContextValue = useMemo(() => ({
    onFileViewRequest: handleFileViewRequest,
    onTabOpen,
    onOpenVisualization,
    onSwitchToChatPanel,
    onToolConfirm: handleToolConfirm,
    onToolReject: handleToolReject,
    sessionId: activeSession?.sessionId,
    activeSessionOverride: activeSession,
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
    onExpandGroup: handleExpandGroup,
    onExpandAllInTurn: handleExpandAllInTurn,
    onCollapseGroup: handleCollapseGroup,
  }), [
    handleFileViewRequest,
    onTabOpen,
    onOpenVisualization,
    onSwitchToChatPanel,
    handleToolConfirm,
    handleToolReject,
    activeSession,
    config,
    exploreGroupStates,
    handleExploreGroupToggle,
    handleExpandGroup,
    handleExpandAllInTurn,
    handleCollapseGroup,
  ]);

  const turnSummaries = useMemo<FlowChatHeaderTurnSummary[]>(() => {
    return (activeSession?.dialogTurns ?? [])
      .filter(turn => !!turn.userMessage)
      .map((turn, index) => ({
        turnId: turn.id,
        turnIndex: index + 1,
        title: turn.userMessage?.content ?? '',
      }));
  }, [activeSession?.dialogTurns]);

  const effectiveVisibleTurnInfo = useMemo<VisibleTurnInfo | null>(() => {
    if (!pendingHeaderTurnId) {
      return visibleTurnInfo;
    }

    const targetTurn = turnSummaries.find(turn => turn.turnId === pendingHeaderTurnId);
    if (!targetTurn) {
      return visibleTurnInfo;
    }

    return {
      turnId: targetTurn.turnId,
      turnIndex: targetTurn.turnIndex,
      totalTurns: turnSummaries.length,
      userMessage: targetTurn.title,
    };
  }, [pendingHeaderTurnId, turnSummaries, visibleTurnInfo]);

  useEffect(() => {
    if (!pendingHeaderTurnId) return;

    if (visibleTurnInfo?.turnId === pendingHeaderTurnId) {
      setPendingHeaderTurnId(null);
      return;
    }

    const targetStillExists = turnSummaries.some(turn => turn.turnId === pendingHeaderTurnId);
    if (!targetStillExists) {
      setPendingHeaderTurnId(null);
    }
  }, [pendingHeaderTurnId, turnSummaries, visibleTurnInfo?.turnId]);

  useEffect(() => {
    autoPinnedSessionIdRef.current = null;
    setPendingHeaderTurnId(null);
  }, [activeSession?.sessionId]);

  useEffect(() => {
    const sessionId = activeSession?.sessionId;
    const latestTurnId = turnSummaries[turnSummaries.length - 1]?.turnId;
    if (!sessionId || !latestTurnId || autoPinnedSessionIdRef.current === sessionId) {
      return;
    }

    const resolvedLatestTurnId = latestTurnId;
    const resolvedSessionId = sessionId;

    autoPinnedSessionIdRef.current = resolvedSessionId;
    setPendingHeaderTurnId(resolvedLatestTurnId);

    const frameId = requestAnimationFrame(() => {
      const accepted = virtualListRef.current?.pinTurnToTop(resolvedLatestTurnId, {
        behavior: 'auto',
        pinMode: 'sticky-latest',
      }) ?? false;

      if (!accepted) {
        autoPinnedSessionIdRef.current = null;
        setPendingHeaderTurnId(null);
      }
    });

    return () => {
      cancelAnimationFrame(frameId);
    };
  }, [activeSession?.sessionId, turnSummaries]);

  const handleJumpToTurn = useCallback((turnId: string) => {
    if (!turnId) return;

    const isLatestTurn = turnSummaries[turnSummaries.length - 1]?.turnId === turnId;

    const accepted = virtualListRef.current?.pinTurnToTop(turnId, {
      behavior: 'smooth',
      pinMode: isLatestTurn ? 'sticky-latest' : 'transient',
    }) ?? false;

    setPendingHeaderTurnId(accepted ? turnId : null);
  }, [turnSummaries]);

  const handleJumpToPreviousTurn = useCallback(() => {
    if (!effectiveVisibleTurnInfo || effectiveVisibleTurnInfo.turnIndex <= 1) return;
    const previousTurn = turnSummaries[effectiveVisibleTurnInfo.turnIndex - 2];
    if (!previousTurn) return;
    handleJumpToTurn(previousTurn.turnId);
  }, [effectiveVisibleTurnInfo, handleJumpToTurn, turnSummaries]);

  const handleJumpToNextTurn = useCallback(() => {
    if (!effectiveVisibleTurnInfo || effectiveVisibleTurnInfo.turnIndex >= turnSummaries.length) return;
    const nextTurn = turnSummaries[effectiveVisibleTurnInfo.turnIndex];
    if (!nextTurn) return;
    handleJumpToTurn(nextTurn.turnId);
  }, [effectiveVisibleTurnInfo, handleJumpToTurn, turnSummaries]);
  
  return (
    <FlowChatContext.Provider value={contextValue}>
      <div className={`modern-flowchat-container ${className}`}>
        <FlowChatHeader
          currentTurn={effectiveVisibleTurnInfo?.turnIndex ?? 0}
          totalTurns={effectiveVisibleTurnInfo?.totalTurns ?? 0}
          currentUserMessage={effectiveVisibleTurnInfo?.userMessage ?? ''}
          visible={virtualItems.length > 0}
          sessionId={activeSession?.sessionId}
          btwOrigin={btwOrigin}
          btwParentTitle={btwParentTitle}
          turns={turnSummaries}
          onJumpToTurn={handleJumpToTurn}
          onJumpToPreviousTurn={handleJumpToPreviousTurn}
          onJumpToNextTurn={handleJumpToNextTurn}
        />

        <div className="modern-flowchat-container__messages">
          {virtualItems.length === 0 ? (
            <WelcomePanel
              key={activeSession?.sessionId ?? 'welcome'}
              sessionMode={activeSession?.mode}
              workspacePath={activeSession?.workspacePath}
              onQuickAction={(command) => {
                window.dispatchEvent(new CustomEvent('fill-chat-input', {
                  detail: { message: command }
                }));
              }}
            />
          ) : (
            <VirtualMessageList
              // Remount per session so Virtuoso does not reuse the previous
              // viewport before the new session's auto-pin settles.
              key={activeSession?.sessionId ?? 'virtual-message-list'}
              ref={virtualListRef}
            />
          )}
        </div>
      </div>
    </FlowChatContext.Provider>
  );
};

ModernFlowChatContainer.displayName = 'ModernFlowChatContainer';
