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
import { CompactToolCard, CompactToolCardHeader } from '../../tool-cards/CompactToolCard';
import { useFlowChatContext } from './FlowChatContext';
import './VirtualItemRenderer.scss';

interface VirtualItemRendererProps {
  item: VirtualItem;
  index: number;
}

export const VirtualItemRenderer = React.memo<VirtualItemRendererProps>(
  ({ item, index }) => {
    const { searchMatchIndices, searchCurrentMatchVirtualIndex } = useFlowChatContext();
    const isSearchMatch = searchMatchIndices != null && searchMatchIndices.size > 0
      ? searchMatchIndices.has(index)
      : false;
    const isSearchCurrent = searchCurrentMatchVirtualIndex != null && searchCurrentMatchVirtualIndex >= 0
      ? searchCurrentMatchVirtualIndex === index
      : false;

    const content = (() => {
      switch (item.type) {
        case 'user-message':
          return <UserMessageItem message={item.data} turnId={item.turnId} />;

        case 'user-steering-message':
          return <UserMessageItem message={item.data} turnId={item.turnId} />;
        
        case 'model-round':
          return (
            <ModelRoundItem 
              round={item.data} 
              turnId={item.turnId} 
              isLastRound={item.isLastRound}
            />
          );
        
        case 'explore-group':
          return (
            <ExploreGroupRenderer
              data={item.data}
              turnId={item.turnId}
            />
          );

        case 'image-analyzing':
          return (
            <div className="model-round-item model-round-item--streaming">
              <CompactToolCard
                status="running"
                header={
                  <CompactToolCardHeader
                    icon={<Loader2 className="animate-spin" size={16} />}
                    content="Analyzing image with image understanding model..."
                  />
                }
              />
            </div>
          );

        default:
          return <div style={{ minHeight: '1px' }} />;
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
        className={wrapperClassName}
        data-turn-id={item.turnId}
        data-item-type={item.type}
      >
        {content || <div style={{ minHeight: '1px' }} />}
      </div>
    );
  },
  (prev, next) => (
    prev.item === next.item &&
    prev.index === next.index
  )
);
VirtualItemRenderer.displayName = 'VirtualItemRenderer';
