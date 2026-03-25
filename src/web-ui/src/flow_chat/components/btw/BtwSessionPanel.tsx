import React, { useEffect, useMemo, useRef, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import path from 'path-browserify';
import { Link2, CornerUpLeft } from 'lucide-react';
import { FlowChatContext } from '../modern/FlowChatContext';
import { VirtualItemRenderer } from '../modern/VirtualItemRenderer';
import { ProcessingIndicator } from '../modern/ProcessingIndicator';
import { flowChatStore } from '../../store/FlowChatStore';
import type { FlowChatConfig, FlowChatState, Session } from '../../types/flow-chat';
import { sessionToVirtualItems } from '../../store/modernFlowChatStore';
import { FLOWCHAT_FOCUS_ITEM_EVENT, type FlowChatFocusItemRequest } from '../../events/flowchatNavigation';
import { fileTabManager } from '@/shared/services/FileTabManager';
import { createTab } from '@/shared/utils/tabUtils';
import { IconButton, type LineRange } from '@/component-library';
import { globalEventBus } from '@/infrastructure/event-bus';
import './BtwSessionPanel.scss';

export interface BtwSessionPanelProps {
  childSessionId?: string;
  parentSessionId?: string;
  workspacePath?: string;
}

const PANEL_CONFIG: FlowChatConfig = {
  enableMarkdown: true,
  autoScroll: true,
  showTimestamps: false,
  maxHistoryRounds: 50,
  enableVirtualScroll: false,
  theme: 'dark',
};

const resolveSessionTitle = (session?: Session | null, fallback = 'Side thread') =>
  session?.title?.trim() || fallback;

export const BtwSessionPanel: React.FC<BtwSessionPanelProps> = ({
  childSessionId,
  parentSessionId,
  workspacePath,
}) => {
  const { t } = useTranslation('flow-chat');
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const shouldAutoScrollRef = useRef(true);

  useEffect(() => {
    const unsubscribe = flowChatStore.subscribe(setFlowChatState);
    return unsubscribe;
  }, []);

  const childSession = childSessionId ? flowChatState.sessions.get(childSessionId) : undefined;
  const parentSession = parentSessionId ? flowChatState.sessions.get(parentSessionId) : undefined;
  const virtualItems = useMemo(() => sessionToVirtualItems(childSession ?? null), [childSession]);

  // Load history for historical sessions that have not yet had their turns loaded.
  const isLoadingRef = useRef(false);
  useEffect(() => {
    if (!childSessionId || !childSession) return;
    if (!childSession.isHistorical) return;
    if (isLoadingRef.current) return;

    const path = workspacePath ?? childSession.workspacePath;
    if (!path) return;

    isLoadingRef.current = true;
    flowChatStore.loadSessionHistory(childSessionId, path).finally(() => {
      isLoadingRef.current = false;
    });
  }, [childSessionId, childSession, workspacePath]);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    const handleWheel = (e: WheelEvent) => {
      if (e.deltaY < 0) {
        shouldAutoScrollRef.current = false;
      } else if (e.deltaY > 0) {
        const { scrollTop, scrollHeight, clientHeight } = container;
        const distanceFromBottom = scrollHeight - scrollTop - clientHeight;
        if (distanceFromBottom < 100) {
          shouldAutoScrollRef.current = true;
        }
      }
    };

    container.addEventListener('wheel', handleWheel, { passive: true });
    return () => container.removeEventListener('wheel', handleWheel);
  }, []);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || !shouldAutoScrollRef.current) return;
    requestAnimationFrame(() => {
      container.scrollTop = container.scrollHeight;
    });
  }, [virtualItems]);

  const handleFileViewRequest = useCallback((
    filePath: string,
    fileName: string,
    lineRange?: LineRange
  ) => {
    let absoluteFilePath = filePath;
    const isWindowsAbsolutePath = /^[A-Za-z]:[\\/]/.test(filePath);

    if (!isWindowsAbsolutePath && !path.isAbsolute(filePath) && workspacePath) {
      absoluteFilePath = path.join(workspacePath, filePath);
    }

    fileTabManager.openFile({
      filePath: absoluteFilePath,
      fileName,
      workspacePath,
      jumpToRange: lineRange,
      mode: 'agent',
    });
  }, [workspacePath]);

  const handleTabOpen = useCallback((tabInfo: any) => {
    if (!tabInfo?.type) return;
    createTab({
      type: tabInfo.type,
      title: tabInfo.title || 'New Tab',
      data: tabInfo.data,
      metadata: tabInfo.metadata,
      checkDuplicate: !!tabInfo.metadata?.duplicateCheckKey,
      duplicateCheckKey: tabInfo.metadata?.duplicateCheckKey,
      replaceExisting: false,
      mode: 'agent',
    });
  }, []);

  const contextValue = useMemo(() => ({
    onFileViewRequest: handleFileViewRequest,
    onTabOpen: handleTabOpen,
    sessionId: childSessionId,
    activeSessionOverride: childSession ?? null,
    config: PANEL_CONFIG,
  }), [childSession, childSessionId, handleFileViewRequest, handleTabOpen]);

  const lastDialogTurn = childSession?.dialogTurns[childSession.dialogTurns.length - 1];
  const lastModelRound = lastDialogTurn?.modelRounds[lastDialogTurn.modelRounds.length - 1];
  const lastItem = lastModelRound?.items[lastModelRound.items.length - 1];
  const lastItemContent = lastItem && 'content' in lastItem ? String((lastItem as any).content || '') : '';
  const isTurnProcessing =
    lastDialogTurn?.status === 'processing' ||
    lastDialogTurn?.status === 'finishing' ||
    lastDialogTurn?.status === 'image_analyzing';
  const [isContentGrowing, setIsContentGrowing] = useState(true);
  const lastContentRef = useRef(lastItemContent);
  const contentTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (lastItemContent !== lastContentRef.current) {
      lastContentRef.current = lastItemContent;
      setIsContentGrowing(true);
      if (contentTimeoutRef.current) clearTimeout(contentTimeoutRef.current);
      contentTimeoutRef.current = setTimeout(() => {
        setIsContentGrowing(false);
      }, 500);
    }

    return () => {
      if (contentTimeoutRef.current) {
        clearTimeout(contentTimeoutRef.current);
      }
    };
  }, [lastItemContent]);

  useEffect(() => {
    if (!isTurnProcessing) {
      setIsContentGrowing(false);
    }
  }, [isTurnProcessing]);

  const showProcessingIndicator = useMemo(() => {
    if (!isTurnProcessing) return false;
    if (!lastItem) return true;

    if (lastItem.type === 'text' || lastItem.type === 'thinking') {
      const hasContent = 'content' in lastItem && Boolean((lastItem as any).content);
      if (hasContent && isContentGrowing) {
        return false;
      }
    }

    if (lastItem.type === 'tool') {
      const toolStatus = (lastItem as any).status;
      if (toolStatus === 'running' || toolStatus === 'streaming' || toolStatus === 'preparing') {
        return false;
      }
    }

    return true;
  }, [isTurnProcessing, lastItem, isContentGrowing]);

  const btwOrigin = childSession?.btwOrigin;
  const parentLabel = resolveSessionTitle(parentSession, t('btw.parent'));
  const backTooltip = btwOrigin?.parentTurnIndex
    ? t('flowChatHeader.btwBackTooltipWithTurn', {
        title: parentLabel,
        turn: btwOrigin.parentTurnIndex,
        defaultValue: `Go back to the source session: ${parentLabel} (Turn ${btwOrigin.parentTurnIndex})`,
      })
    : t('flowChatHeader.btwBackTooltipWithoutTurn', {
        title: parentLabel,
        defaultValue: `Go back to the source session: ${parentLabel}`,
      });

  const handleFocusOriginTurn = useCallback(() => {
    const resolvedParentSessionId = btwOrigin?.parentSessionId || parentSessionId;
    if (!resolvedParentSessionId) return;

    const requestId = btwOrigin?.requestId;
    const itemId = requestId ? `btw_marker_${requestId}` : undefined;
    const request: FlowChatFocusItemRequest = {
      sessionId: resolvedParentSessionId,
      turnIndex: btwOrigin?.parentTurnIndex,
      itemId,
      source: 'btw-back',
    };

    globalEventBus.emit(
      FLOWCHAT_FOCUS_ITEM_EVENT,
      request,
      'BtwSessionPanel'
    );
  }, [btwOrigin, parentSessionId]);

  if (!childSessionId || !childSession) {
    return (
      <div className="btw-session-panel btw-session-panel--empty">
        <div className="btw-session-panel__empty-state">
          {t('btw.emptyThreadLabel', { label: t('btw.threadLabel') })}
        </div>
      </div>
    );
  }

  return (
    <FlowChatContext.Provider value={contextValue}>
      <div className="btw-session-panel">
        <div className="btw-session-panel__header">
          <div className="btw-session-panel__header-left">
            <span className="btw-session-panel__badge">{t('btw.shortLabel')}</span>
          </div>
          <div className="btw-session-panel__header-title-wrap">
            <span className="btw-session-panel__title">{resolveSessionTitle(childSession, t('btw.threadLabel'))}</span>
          </div>
          <div className="btw-session-panel__header-right">
            <div className="btw-session-panel__meta">
              <span className="btw-session-panel__meta-label">{t('btw.origin')}</span>
              <Link2 size={11} />
              <span className="btw-session-panel__meta-title">{resolveSessionTitle(parentSession, t('btw.parent'))}</span>
            </div>
            {!!(btwOrigin?.parentSessionId || parentSessionId) && (
              <IconButton
                className="btw-session-panel__origin-button"
                variant="ghost"
                size="xs"
                onClick={handleFocusOriginTurn}
                tooltip={backTooltip}
                aria-label={t('btw.backToParent')}
                data-testid="btw-session-panel-origin-button"
              >
                <CornerUpLeft size={12} />
              </IconButton>
            )}
          </div>
        </div>

        <div ref={scrollContainerRef} className="btw-session-panel__body">
          {virtualItems.length === 0 ? (
            <div className="btw-session-panel__empty-state">{t('session.empty')}</div>
          ) : (
            <>
              {virtualItems.map((item, index) => (
                <VirtualItemRenderer
                  key={`${item.turnId}-${item.type}-${index}`}
                  item={item}
                  index={index}
                />
              ))}
              <ProcessingIndicator
                visible={showProcessingIndicator}
                reserveSpace={isTurnProcessing}
              />
            </>
          )}
        </div>
      </div>
    </FlowChatContext.Provider>
  );
};

BtwSessionPanel.displayName = 'BtwSessionPanel';
