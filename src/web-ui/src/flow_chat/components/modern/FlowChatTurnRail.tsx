import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import { observeElementResize } from '@/shared/utils/sharedResizeObserver';
import './FlowChatTurnRail.scss';

export interface FlowChatTurnRailItem {
  turnId: string;
  turnIndex: number;
  content: string;
}

interface FlowChatTurnRailProps {
  turns: readonly FlowChatTurnRailItem[];
  currentTurnId: string | null;
  visibleTurnIds: readonly string[];
  onNavigate: (turnId: string) => void;
}

export const FlowChatTurnRail: React.FC<FlowChatTurnRailProps> = ({
  turns,
  currentTurnId,
  visibleTurnIds,
  onNavigate,
}) => {
  const { t } = useTranslation('flow-chat');
  const railRef = useRef<HTMLElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);
  const itemRefs = useRef(new Map<string, HTMLButtonElement>());
  const [focusTurnId, setFocusTurnId] = useState<string | null>(
    currentTurnId ?? turns[0]?.turnId ?? null,
  );
  const visibleTurnIdSet = useMemo(() => new Set(visibleTurnIds), [visibleTurnIds]);

  useEffect(() => {
    const focusTurnStillExists = focusTurnId !== null && turns.some(turn => turn.turnId === focusTurnId);
    if (!focusTurnStillExists) {
      setFocusTurnId(currentTurnId ?? turns[0]?.turnId ?? null);
      return;
    }

    if (
      currentTurnId &&
      railRef.current &&
      !railRef.current.contains(document.activeElement)
    ) {
      setFocusTurnId(currentTurnId);
    }
  }, [currentTurnId, focusTurnId, turns]);

  const keepCurrentTurnVisible = useCallback(() => {
    if (!currentTurnId) return;

    const list = listRef.current;
    const activeItem = itemRefs.current.get(currentTurnId);
    if (!list || !activeItem || list.clientHeight <= 0) return;

    const itemTop = activeItem.offsetTop;
    const itemBottom = itemTop + activeItem.offsetHeight;
    const visibleTop = list.scrollTop;
    const visibleBottom = visibleTop + list.clientHeight;

    if (itemTop < visibleTop) {
      list.scrollTop = itemTop;
    } else if (itemBottom > visibleBottom) {
      list.scrollTop = itemBottom - list.clientHeight;
    }
  }, [currentTurnId]);

  useLayoutEffect(() => {
    keepCurrentTurnVisible();
  }, [keepCurrentTurnVisible, turns.length]);

  useEffect(() => {
    const list = listRef.current;
    if (!list) return;

    return observeElementResize(list, keepCurrentTurnVisible);
  }, [keepCurrentTurnVisible]);

  const focusTurnAt = useCallback((index: number) => {
    const turn = turns[index];
    if (!turn) return;

    setFocusTurnId(turn.turnId);
    itemRefs.current.get(turn.turnId)?.focus();
  }, [turns]);

  const handleKeyDown = useCallback((event: React.KeyboardEvent, index: number) => {
    let nextIndex: number | null = null;

    switch (event.key) {
      case 'ArrowUp':
        nextIndex = Math.max(0, index - 1);
        break;
      case 'ArrowDown':
        nextIndex = Math.min(turns.length - 1, index + 1);
        break;
      case 'Home':
        nextIndex = 0;
        break;
      case 'End':
        nextIndex = turns.length - 1;
        break;
      default:
        return;
    }

    event.preventDefault();
    event.stopPropagation();
    focusTurnAt(nextIndex);
  }, [focusTurnAt, turns.length]);

  if (turns.length === 0) return null;

  const navigationLabel = t('flowChatTurnRail.label');
  const untitledTurnLabel = t('flowChatTurnRail.untitledTurn');

  return (
    <nav
      ref={railRef}
      className="flowchat-turn-rail"
      aria-label={navigationLabel}
      data-testid="flowchat-turn-rail"
    >
      <div ref={listRef} className="flowchat-turn-rail__list">
        {turns.map((turn, index) => {
          const isCurrent = turn.turnId === currentTurnId;
          const isVisible = visibleTurnIdSet.has(turn.turnId);
          const turnLabel = t('flowChatHeader.turnBadge', { current: turn.turnIndex });
          const content = turn.content.trim() || untitledTurnLabel;

          return (
            <Tooltip
              key={turn.turnId}
              placement="right"
              delay={250}
              className="flowchat-turn-rail__tooltip"
              content={(
                <span className="flowchat-turn-rail__tooltip-content">
                  <span className="flowchat-turn-rail__tooltip-turn">{turnLabel}</span>
                  <span className="flowchat-turn-rail__tooltip-message">{content}</span>
                </span>
              )}
            >
              <button
                ref={(node) => {
                  if (node) {
                    itemRefs.current.set(turn.turnId, node);
                  } else {
                    itemRefs.current.delete(turn.turnId);
                  }
                }}
                type="button"
                className={`flowchat-turn-rail__item${isVisible ? ' flowchat-turn-rail__item--visible' : ''}`}
                aria-label={turnLabel}
                aria-current={isCurrent ? 'step' : undefined}
                tabIndex={turn.turnId === focusTurnId ? 0 : -1}
                data-turn-id={turn.turnId}
                data-turn-index={turn.turnIndex}
                onClick={() => onNavigate(turn.turnId)}
                onFocus={() => setFocusTurnId(turn.turnId)}
                onKeyDown={(event) => handleKeyDown(event, index)}
              >
                <span className="flowchat-turn-rail__bar" aria-hidden="true" />
              </button>
            </Tooltip>
          );
        })}
      </div>
    </nav>
  );
};

FlowChatTurnRail.displayName = 'FlowChatTurnRail';
