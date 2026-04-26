/**
 * Terminal group renderer.
 * Renders merged consecutive Bash commands as a collapsible region.
 */

import React, { useRef, useMemo, useCallback, useEffect, useState } from 'react';
import { ChevronRight, Terminal } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { FlowToolItem } from '../../types/flow-chat';
import { FlowToolCard } from '../FlowToolCard';
import { useToolCardHeightContract } from '../../tool-cards/useToolCardHeightContract';
import { useFlowChatContext } from './FlowChatContext';
import './TerminalGroupRenderer.scss';

export interface TerminalGroupRendererProps {
  items: FlowToolItem[];
  turnId: string;
  roundId: string;
  isLast?: boolean;
  isGroupStreaming?: boolean;
}

interface TerminalGroupStats {
  total: number;
  success: number;
  failed: number;
  running: number;
  cancelled: number;
}

function computeTerminalStats(items: FlowToolItem[]): TerminalGroupStats {
  const stats: TerminalGroupStats = { total: items.length, success: 0, failed: 0, running: 0, cancelled: 0 };
  for (const item of items) {
    const status = item.status || 'pending';
    if (status === 'completed') {
      const result = item.toolResult?.result;
      let exitCode = 0;
      if (typeof result === 'string') {
        try {
          const parsed = JSON.parse(result);
          exitCode = typeof parsed.exit_code === 'number' ? parsed.exit_code : 0;
        } catch {
          exitCode = 0;
        }
      } else if (result && typeof result === 'object') {
        exitCode = typeof (result as any).exit_code === 'number' ? (result as any).exit_code : 0;
      }
      if (exitCode === 0) {
        stats.success++;
      } else {
        stats.failed++;
      }
    } else if (status === 'error') {
      stats.failed++;
    } else if (status === 'running' || status === 'streaming' || status === 'preparing') {
      stats.running++;
    } else if (status === 'cancelled') {
      stats.cancelled++;
    }
  }
  return stats;
}

export const TerminalGroupRenderer: React.FC<TerminalGroupRendererProps> = React.memo(({
  items,
  turnId: _turnId,
  roundId: _roundId,
  isLast: _isLast,
  isGroupStreaming = false,
}) => {
  const { t } = useTranslation('flow-chat');
  const containerRef = useRef<HTMLDivElement>(null);
  const [scrollState, setScrollState] = useState({ hasScroll: false, atTop: true, atBottom: true });

  const {
    terminalGroupStates,
    onTerminalGroupToggle,
    onExpandTerminalGroup,
    onCollapseTerminalGroup,
  } = useFlowChatContext();

  const groupId = useMemo(() => `terminal-group-${items.map((it) => it.id).join('-')}`, [items]);
  const stats = useMemo(() => computeTerminalStats(items), [items]);
  const wasStreamingRef = useRef(isGroupStreaming);

  const {
    cardRootRef,
    applyExpandedState,
  } = useToolCardHeightContract({
    toolId: groupId,
    toolName: 'terminal-group',
    getCardHeight: () => (
      containerRef.current?.scrollHeight
      ?? containerRef.current?.getBoundingClientRect().height
      ?? null
    ),
  });

  const hasExplicitState = terminalGroupStates?.has(groupId) ?? false;
  const explicitExpanded = terminalGroupStates?.get(groupId) ?? false;
  const isExpanded = hasExplicitState ? explicitExpanded : isGroupStreaming;
  const isCollapsed = !isExpanded;
  const allowManualToggle = !isGroupStreaming;

  const checkScrollState = useCallback(() => {
    const el = containerRef.current;
    if (!el) {
      return;
    }

    setScrollState({
      hasScroll: el.scrollHeight > el.clientHeight + 1,
      atTop: el.scrollTop <= 5,
      atBottom: el.scrollTop + el.clientHeight >= el.scrollHeight - 5,
    });
  }, []);

  useEffect(() => {
    if (isGroupStreaming && !hasExplicitState) {
      applyExpandedState(false, true, () => {
        onExpandTerminalGroup?.(groupId);
      });
      wasStreamingRef.current = true;
      return;
    }

    if (wasStreamingRef.current && !isGroupStreaming && isExpanded) {
      applyExpandedState(true, false, () => {
        onCollapseTerminalGroup?.(groupId);
      }, {
        reason: 'auto',
      });
    }

    wasStreamingRef.current = isGroupStreaming;
  }, [
    applyExpandedState,
    groupId,
    hasExplicitState,
    isExpanded,
    isGroupStreaming,
    onCollapseTerminalGroup,
    onExpandTerminalGroup,
  ]);

  useEffect(() => {
    if (!isCollapsed && isGroupStreaming && containerRef.current) {
      requestAnimationFrame(() => {
        if (containerRef.current) {
          containerRef.current.scrollTop = containerRef.current.scrollHeight;
          checkScrollState();
        }
      });
    }
  }, [items, checkScrollState, isCollapsed, isGroupStreaming]);

  useEffect(() => {
    if (!isExpanded) {
      setScrollState({ hasScroll: false, atTop: true, atBottom: true });
      return;
    }

    const el = containerRef.current;
    if (!el) {
      return;
    }

    const frameId = requestAnimationFrame(checkScrollState);

    if (typeof ResizeObserver === 'undefined') {
      return () => cancelAnimationFrame(frameId);
    }

    const observer = new ResizeObserver(() => {
      checkScrollState();
    });
    observer.observe(el);

    return () => {
      cancelAnimationFrame(frameId);
      observer.disconnect();
    };
  }, [items, checkScrollState, isExpanded]);

  const displaySummary = useMemo(() => {
    const { total, success, failed, running, cancelled } = stats;
    const parts: string[] = [];

    parts.push(t('terminalRegion.executedCommands', { count: total }));

    if (failed === 0 && running === 0 && cancelled === 0) {
      parts.push(t('terminalRegion.allSuccess'));
    } else if (running > 0) {
      if (failed > 0) {
        parts.push(t('terminalRegion.mixedStatus', { success, failed, running }));
      } else {
        parts.push(t('terminalRegion.withRunning', { success, running }));
      }
    } else if (cancelled > 0 && failed === 0) {
      parts.push(t('terminalRegion.withCancelled', { success, cancelled }));
    } else if (failed > 0) {
      parts.push(t('terminalRegion.partialSuccess', { success, failed }));
    }

    return parts.join(t('terminalRegion.separator'));
  }, [stats, t]);

  const statusDotClass = useMemo(() => {
    const { failed, running } = stats;
    if (running > 0) return 'terminal-region__status-dot--running';
    if (failed > 0) return 'terminal-region__status-dot--failed';
    return 'terminal-region__status-dot--success';
  }, [stats]);

  const handleToggle = useCallback(() => {
    if (isCollapsed) {
      applyExpandedState(false, true, () => {
        onTerminalGroupToggle?.(groupId);
      });
      return;
    }

    applyExpandedState(true, false, () => {
      onCollapseTerminalGroup?.(groupId);
    });
  }, [applyExpandedState, groupId, isCollapsed, onCollapseTerminalGroup, onTerminalGroupToggle]);

  const className = [
    'terminal-region',
    allowManualToggle ? 'terminal-region--collapsible' : null,
    isCollapsed ? 'terminal-region--collapsed' : 'terminal-region--expanded',
    isGroupStreaming ? 'terminal-region--streaming' : null,
    scrollState.hasScroll ? 'terminal-region--has-scroll' : null,
    scrollState.atTop ? 'terminal-region--at-top' : null,
    scrollState.atBottom ? 'terminal-region--at-bottom' : null,
  ].filter(Boolean).join(' ');

  return (
    <div
      ref={cardRootRef}
      data-tool-card-id={groupId}
      className={className}
    >
      {allowManualToggle && (
        <div className="terminal-region__header" onClick={handleToggle}>
          <ChevronRight size={14} className="terminal-region__icon" />
          <Terminal size={14} className="terminal-region__tool-icon" />
          <span className="terminal-region__summary">{displaySummary}</span>
          <span className={`terminal-region__status-dot ${statusDotClass}`} />
        </div>
      )}
      <div className="terminal-region__content-wrapper">
        <div className="terminal-region__content-inner">
          <div ref={containerRef} className="terminal-region__content" onScroll={checkScrollState}>
            {items.map((item) => (
              <FlowToolCard
                key={item.id}
                toolItem={item}
              />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
});

TerminalGroupRenderer.displayName = 'TerminalGroupRenderer';
