 

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button, Icon, ScrollArea } from '@bitfun/ui';
import { AlertCircle } from 'lucide-react';
import { useContextStore, selectContexts } from '../../../stores/contextStore';
import { ContextCard } from '../ContextCard/ContextCard';
import { useI18n } from '@/infrastructure/i18n';
import { isReducedMotionPreferred } from '@/shared/utils/motionPreference';
import './ContextList.scss';

export interface ContextListProps {
  compact?: boolean;
  interactive?: boolean;
  showPreview?: boolean;
  maxHeight?: string;
  className?: string;
  onContextClick?: (contextId: string) => void;
}

const CONTEXT_ROW_EXIT_MS = 160;

export const ContextList: React.FC<ContextListProps> = ({
  compact = false,
  interactive = true,
  showPreview = true,
  maxHeight = '200px',
  className = '',
  onContextClick
}) => {
  const { t } = useI18n('components');
  const contexts = useContextStore(selectContexts);
  const removeContext = useContextStore(state => state.removeContext);
  const [leavingContextIds, setLeavingContextIds] = useState<Set<string>>(new Set());
  const exitTimersRef = useRef<Map<string, number>>(new Map());

  useEffect(() => () => {
    const pendingContextIds = [...exitTimersRef.current.keys()];
    exitTimersRef.current.forEach(timer => window.clearTimeout(timer));
    exitTimersRef.current.clear();
    pendingContextIds.forEach(removeContext);
  }, [removeContext]);

  const removeAfterExit = useCallback((contextIds: string[]) => {
    if (isReducedMotionPreferred()) {
      contextIds.forEach(removeContext);
      return;
    }

    setLeavingContextIds(previous => {
      const next = new Set(previous);
      contextIds.forEach(id => next.add(id));
      return next;
    });

    contextIds.forEach((contextId) => {
      const existingTimer = exitTimersRef.current.get(contextId);
      if (existingTimer) window.clearTimeout(existingTimer);
      const timer = window.setTimeout(() => {
        exitTimersRef.current.delete(contextId);
        removeContext(contextId);
        setLeavingContextIds(previous => {
          const next = new Set(previous);
          next.delete(contextId);
          return next;
        });
      }, CONTEXT_ROW_EXIT_MS);
      exitTimersRef.current.set(contextId, timer);
    });
  }, [removeContext]);
  
  const handleRemove = useCallback((id: string) => {
    if (!contexts.some(item => item.id === id) || leavingContextIds.has(id)) return;
    removeAfterExit([id]);
  }, [contexts, leavingContextIds, removeAfterExit]);
  
  const handleClearAll = useCallback(async () => {
    
    if (
      leavingContextIds.size === 0
      && await window.confirm(t('contextSystem.contextList.clearAllConfirm', { count: contexts.length }))
    ) {
      removeAfterExit(contexts.map(context => context.id));
    }
  }, [contexts, leavingContextIds.size, removeAfterExit, t]);
  
  const handleCardClick = useCallback((contextId: string) => {
    onContextClick?.(contextId);
  }, [onContextClick]);

  const renderedContexts = useMemo(() => {
    return contexts.map(context => ({
      context,
      exiting: leavingContextIds.has(context.id),
    }));
  }, [contexts, leavingContextIds]);
  
  if (renderedContexts.length === 0) {
    return (
      <div className={`bitfun-context-list bitfun-context-list--empty ${className}`} data-bf-component="context-list" data-bf-part="root" data-bf-state="empty">
        <div className="bitfun-context-list__empty-state" data-bf-component="context-list" data-bf-part="empty">
          <AlertCircle size={24} className="bitfun-context-list__empty-icon" />
          <p className="bitfun-context-list__empty-text">
            {t('contextSystem.contextList.emptyTitle')}
          </p>
          <p className="bitfun-context-list__empty-hint">
            {t('contextSystem.contextList.emptyHint')}
          </p>
        </div>
      </div>
    );
  }
  
  return (
    <div className={`bitfun-context-list ${className}`} data-bf-component="context-list" data-bf-part="root">
      
      <div className="bitfun-context-list__header" data-bf-component="context-list" data-bf-part="header">
        <div className="bitfun-context-list__title" data-bf-component="context-list" data-bf-part="title">
          {t('contextSystem.contextList.title')}
          <span className="bitfun-context-list__count" data-bf-component="context-list" data-bf-part="count">
            {contexts.length}
          </span>
        </div>
        
        <Button
          type="button"
          variant="outline"
          size="sm"
          leadingIcon={<Icon name="xmark" size="sm" />}
          onClick={handleClearAll}
          disabled={leavingContextIds.size > 0}
          title={t('contextSystem.contextList.clearAllTitle')}
        >
          {t('contextSystem.contextList.clearAll')}
        </Button>
      </div>
      
      
      <ScrollArea 
        className="bitfun-context-list__items"
        style={{ maxHeight }}
        data-bf-component="context-list"
        data-bf-part="items"
      >
        {renderedContexts.map(({ context, exiting }) => (
          <div
            key={context.id}
            className="bitfun-context-list__item"
            data-exiting={exiting ? 'true' : 'false'}
            aria-hidden={exiting || undefined}
            {...(exiting ? { inert: '' } : {})}
            onClick={exiting ? undefined : () => handleCardClick(context.id)}
            data-bf-component="context-list"
            data-bf-part="item"
          >
            <div className="bitfun-context-list__item-inner">
              <ContextCard
                context={context}
                onRemove={exiting ? undefined : handleRemove}
                compact={compact}
                interactive={interactive && !exiting}
                showPreview={showPreview}
              />
            </div>
          </div>
        ))}
      </ScrollArea>
    </div>
  );
};

export default ContextList;
