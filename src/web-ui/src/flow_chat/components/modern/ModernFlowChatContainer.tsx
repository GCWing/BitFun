/**
 * Modern FlowChat container.
 * Uses virtual scrolling with Zustand and syncs legacy store state.
 */

import React, { useMemo, useCallback, useRef, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { FlowChatManager } from '@/flow_chat/services/FlowChatManager';
import { useSessionModeStore } from '@/app/stores/sessionModeStore';
import { VirtualMessageList, VirtualMessageListRef } from './VirtualMessageList';
import { FlowChatHeader, type FlowChatHeaderTurnSummary } from './FlowChatHeader';
import { WelcomePanel } from '../WelcomePanel';
import { FlowChatContext, FlowChatContextValue } from './FlowChatContext';
import { useExploreGroupState } from './useExploreGroupState';
import { useFlowChatFileActions } from './useFlowChatFileActions';
import { useFlowChatNavigation } from './useFlowChatNavigation';
import { useFlowChatCopyDialog } from './useFlowChatCopyDialog';
import { useFlowChatSync } from './useFlowChatSync';
import { useFlowChatToolActions } from './useFlowChatToolActions';
import { useFlowChatSearch } from './useFlowChatSearch';
import { useVirtualItems, useActiveSession, useVisibleTurnInfo, type VisibleTurnInfo } from '../../store/modernFlowChatStore';
import type { FlowChatConfig } from '../../types/flow-chat';
import type { LineRange } from '@/component-library';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { isAcpFlowSession } from '../../utils/acpSession';
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
  const { t } = useTranslation('flow-chat');
  const virtualItems = useVirtualItems();
  const activeSession = useActiveSession();
  const visibleTurnInfo = useVisibleTurnInfo();
  const [pendingHeaderTurnId, setPendingHeaderTurnId] = useState<string | null>(null);
  const [searchOpenRequest, setSearchOpenRequest] = useState(0);
  const autoPinnedSessionIdRef = useRef<string | null>(null);
  const virtualListRef = useRef<VirtualMessageListRef>(null);
  const chatScopeRef = useRef<HTMLDivElement>(null);
  const { workspacePath } = useWorkspaceContext();
  const allowUserMessageRollback = !isAcpFlowSession(activeSession);
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
  const {
    searchQuery,
    onSearchChange,
    matches: searchMatches,
    matchIndices: searchMatchIndices,
    currentMatchIndex: searchCurrentMatchIndex,
    currentMatchVirtualIndex: searchCurrentMatchVirtualIndex,
    goToNext: handleSearchNext,
    goToPrev: handleSearchPrev,
    clearSearch,
  } = useFlowChatSearch(virtualItems);

  useFlowChatSync();
  useFlowChatCopyDialog();

  useFlowChatNavigation({
    activeSessionId: activeSession?.sessionId,
    virtualItems,
    virtualListRef,
    onExpandExploreGroup: handleExpandGroup,
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
    allowUserMessageRollback,
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
    searchQuery,
    searchMatchIndices,
    searchCurrentMatchVirtualIndex,
  }), [
    handleFileViewRequest,
    onTabOpen,
    onOpenVisualization,
    onSwitchToChatPanel,
    handleToolConfirm,
    handleToolReject,
    activeSession,
    allowUserMessageRollback,
    config,
    exploreGroupStates,
    handleExploreGroupToggle,
    handleExpandGroup,
    handleExpandAllInTurn,
    handleCollapseGroup,
    searchQuery,
    searchMatchIndices,
    searchCurrentMatchVirtualIndex,
  ]);

  const turnSummaries = useMemo<FlowChatHeaderTurnSummary[]>(() => {
    return (activeSession?.dialogTurns ?? [])
      .filter(turn => !!turn.userMessage)
      .map((turn, index) => ({
        turnId: turn.id,
        turnIndex: index + 1,
        title: turn.userMessage?.metadata?.localCommandKind === 'usage_report'
          ? t('usage.title')
          : turn.userMessage?.content ?? '',
      }));
  }, [activeSession?.dialogTurns, t]);

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

  const currentHeaderMessage = useMemo(() => {
    const turnId = effectiveVisibleTurnInfo?.turnId;
    if (!turnId) {
      return effectiveVisibleTurnInfo?.userMessage ?? '';
    }
    const turn = activeSession?.dialogTurns.find(item => item.id === turnId);
    if (turn?.userMessage?.metadata?.localCommandKind === 'usage_report') {
      return t('usage.title');
    }
    return effectiveVisibleTurnInfo?.userMessage ?? '';
  }, [activeSession?.dialogTurns, effectiveVisibleTurnInfo?.turnId, effectiveVisibleTurnInfo?.userMessage, t]);

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

  useEffect(() => {
    if (searchCurrentMatchVirtualIndex < 0) return;
    const frameId = requestAnimationFrame(() => {
      virtualListRef.current?.scrollToIndex(searchCurrentMatchVirtualIndex);
    });
    return () => {
      cancelAnimationFrame(frameId);
    };
  }, [searchCurrentMatchVirtualIndex]);

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

  useShortcut(
    'chat.stopGeneration',
    { key: 'Escape', scope: 'chat', allowInInput: true },
    () => {
      void FlowChatManager.getInstance().cancelCurrentTask();
    },
    { priority: 20, description: 'keyboard.shortcuts.chat.stopGeneration' }
  );

  useShortcut(
    'chat.newSession',
    { key: 'N', ctrl: true, scope: 'chat' },
    () => {
      void (async () => {
        try {
          useSessionModeStore.getState().setMode('code');
          await FlowChatManager.getInstance().createChatSession({}, 'agentic');
        } catch {
          /* ignore */
        }
      })();
    },
    { priority: 10, description: 'keyboard.shortcuts.chat.newSession' }
  );

  useShortcut(
    'btw-fill',
    { key: 'B', ctrl: true, alt: true, scope: 'chat', allowInInput: true },
    () => {
      const selected = (window.getSelection?.()?.toString() ?? '').trim();
      const message = selected ? `/btw Explain this:\n\n${selected}` : '/btw ';
      window.dispatchEvent(new CustomEvent('fill-chat-input', { detail: { message } }));
    },
    { priority: 20, description: 'keyboard.shortcuts.chat.btwFill' }
  );

  useShortcut(
    'chat.search',
    { key: 'F', ctrl: true, scope: 'chat', allowInInput: false },
    () => {
      setSearchOpenRequest(prev => prev + 1);
    },
    { priority: 15, description: 'keyboard.shortcuts.chat.search' }
  );

  return (
    <FlowChatContext.Provider value={contextValue}>
      <div
        ref={chatScopeRef}
        className={`modern-flowchat-container flow-chat-typography ${className}`}
        data-shortcut-scope="chat"
      >
        <FlowChatHeader
          currentTurn={effectiveVisibleTurnInfo?.turnIndex ?? 0}
          totalTurns={effectiveVisibleTurnInfo?.totalTurns ?? 0}
          currentUserMessage={currentHeaderMessage}
          visible={virtualItems.length > 0}
          sessionId={activeSession?.sessionId}
          turns={turnSummaries}
          onJumpToTurn={handleJumpToTurn}
          onJumpToCurrentTurn={() => {
            const turnId = effectiveVisibleTurnInfo?.turnId;
            if (turnId) handleJumpToTurn(turnId);
          }}
          onJumpToPreviousTurn={handleJumpToPreviousTurn}
          onJumpToNextTurn={handleJumpToNextTurn}
          searchQuery={searchQuery}
          onSearchChange={onSearchChange}
          searchMatchCount={searchMatches.length}
          searchCurrentMatch={searchMatches.length > 0 ? searchCurrentMatchIndex + 1 : 0}
          onSearchNext={handleSearchNext}
          onSearchPrev={handleSearchPrev}
          onSearchClose={clearSearch}
          searchOpenRequest={searchOpenRequest}
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
