/**
 * Virtual item renderer.
 * Renders user messages, model rounds, explore groups, or image-analyzing indicators by type.
 */

import React from 'react';
import { Loader2 } from 'lucide-react';
import type { VirtualItem } from '../../store/modernFlowChatStore';
import { UserMessageItem } from './UserMessageItem';
import { ModelRoundItem } from './ModelRoundItem';
import { ExploreGroupRenderer } from './ExploreGroupRenderer';
import { AmbientToolCard, AmbientToolCardHeader } from '@bitfun/ui/flow-chat';
import { useFlowChatVolatileContext } from './FlowChatContext';
import { TurnCompletionNoticeItem } from './TurnCompletionNoticeItem';
import { TurnFailureNoticeItem } from './TurnFailureNoticeItem';
import './VirtualItemRenderer.scss';
import { getVirtualItemStableKey } from './virtualItemIdentity';

interface VirtualItemRendererProps {
  item: VirtualItem;
  index: number;
  /**
   * Ref callback the virtualizer measures this item through.
   *
   * It reads `data-virtual-index` back off the element, so the attribute below
   * and this callback have to land on the same node.
   */
  measureRef?: (element: HTMLElement | null) => void;
}

export const VirtualItemRenderer = React.memo<VirtualItemRendererProps>(
  ({ item, index, measureRef }) => {
    const { searchMatchIndices, searchCurrentMatchVirtualIndex } = useFlowChatVolatileContext();
    const isSearchMatch = searchMatchIndices != null && searchMatchIndices.size > 0
      ? searchMatchIndices.has(index)
      : false;
    const isSearchCurrent = searchCurrentMatchVirtualIndex != null && searchCurrentMatchVirtualIndex >= 0
      ? searchCurrentMatchVirtualIndex === index
      : false;

    const content = (() => {
      switch (item.type) {
        case 'user-message':
          return (
            <UserMessageItem
              message={item.data}
              turnId={item.turnId}
              absoluteTurnIndex={item.absoluteTurnIndex}
              turnStatus={item.turnStatus}
            />
          );

        case 'user-steering-message':
          return (
            <UserMessageItem
              message={item.data}
              turnId={item.turnId}
              steeringStatus={item.steeringStatus}
            />
          );
        
        case 'model-round':
          return (
            <ModelRoundItem 
              round={item.data} 
              turnId={item.turnId} 
              isLastRound={item.isLastRound}
              isTurnComplete={item.isTurnComplete}
              turnStartedAt={item.turnStartedAt}
              turnEndedAt={item.turnEndedAt}
              turnDurationMs={item.turnDurationMs}
              turnTokenUsage={item.turnTokenUsage}
              expandedThinkingItemIds={item.layoutHints?.expandedThinkingItemIds ?? []}
            />
          );
        
        case 'explore-group':
          return (
            <ExploreGroupRenderer
              data={item.data}
              turnId={item.turnId}
            />
          );

        case 'turn-completion-notice':
          return <TurnCompletionNoticeItem notice={item.data} />;

        case 'turn-failure-notice':
          return <TurnFailureNoticeItem error={item.data.error} errorDetail={item.data.errorDetail} />;

        case 'image-analyzing':
          return (
            <div data-bf-component="virtual-item" data-bf-part="imageAnalyzing" className="model-round-item model-round-item--streaming">
              <AmbientToolCard
                status="running"
                header={
                  <AmbientToolCardHeader
                    icon={<Loader2 className="animate-spin" size={16} />}
                    content="Analyzing image with image understanding model..."
                  />
                }
              />
            </div>
          );

        default:
          return <div data-bf-component="virtual-item" data-bf-part="placeholder" style={{ minHeight: '1px' }} />;
      }
    })();
    
    // A4-like layout: wrap with a max-width container.
    // Render the container even when content is empty to avoid zero-size issues.
    // data-turn-id is used for long-image export.
    const wrapperClassName = [
      'virtual-item-wrapper',
      isSearchCurrent ? 'virtual-item-wrapper--search-current' : isSearchMatch ? 'virtual-item-wrapper--search-match' : '',
    ].filter(Boolean).join(' ');

    return (
      <div
        ref={measureRef}
        data-bf-component="virtual-item"
        data-bf-part="root"
        data-bf-state={[isSearchMatch && 'searchMatch', isSearchCurrent && 'searchCurrent'].filter(Boolean).join(' ')}
        className={wrapperClassName}
        data-testid="flowchat-message-item"
        data-turn-id={item.turnId}
        data-item-type={item.type}
        data-virtual-item-key={getVirtualItemStableKey(item)}
        data-virtual-index={index}
        data-item-index={index}
      >
        {content || <div style={{ minHeight: '1px' }} />}
      </div>
    );
  },
  (prev, next) => (
    prev.item === next.item &&
    prev.index === next.index &&
    prev.measureRef === next.measureRef
  )
);
VirtualItemRenderer.displayName = 'VirtualItemRenderer';
