/**
 * Streaming item renderer
 * Dispatches to the right component by item type
 * Uses React.memo to avoid unnecessary re-renders
 */

import React from 'react';
import { FlowItem, FlowTextItem, FlowToolItem, FlowThinkingItem, FlowUserSteeringItem } from '../types/flow-chat';
import { FlowTextBlock } from './FlowTextBlock';
import { FlowToolCard } from './FlowToolCard';
import { ModelThinkingDisplay } from '../tool-cards/ModelThinkingDisplay';
import { UserSteeringBubble } from './UserSteeringBubble';

interface FlowItemRendererProps {
  item: FlowItem;
  onFileViewRequest?: (filePath: string) => void;
  onTabOpen?: (tabInfo: any) => void;
  onConfirm?: (toolId: string, updatedInput?: any, permissionOptionId?: string, approve?: boolean) => void;
  onReject?: (toolId: string, permissionOptionId?: string) => void;
  sessionId?: string;
}

const FlowItemRendererComponent: React.FC<FlowItemRendererProps> = ({
  item,
  onFileViewRequest,
  onTabOpen,
  onConfirm,
  onReject,
  sessionId
}) => {
  if (item.type === 'text') {
    const textItem = item as FlowTextItem;
    return <FlowTextBlock textItem={textItem} />;
  }
  
  if (item.type === 'thinking') {
    return <ModelThinkingDisplay thinkingItem={item as FlowThinkingItem} />;
  }
  
  if (item.type === 'user-steering') {
    return <UserSteeringBubble item={item as FlowUserSteeringItem} />;
  }

  if (item.type === 'tool') {
    const toolItem = item as FlowToolItem;
    return (
      <div className="flowchat-tool-wrapper">
        <FlowToolCard
          toolItem={toolItem}
          onConfirm={onConfirm}
          onReject={onReject}
          onOpenInEditor={onFileViewRequest}
          onOpenInPanel={onTabOpen}
          sessionId={sessionId}
        />
      </div>
    );
  }
  
  return null;
};

// Key optimization: React.memo
export const FlowItemRenderer = React.memo(
  FlowItemRendererComponent,
  (prev, next) => {
    // Re-render if ID changes
    if (prev.item.id !== next.item.id) return false;
    
    // Re-render if status changes
    if (prev.item.status !== next.item.status) return false;
    
    // Compare text content for text items
    if (prev.item.type === 'text' && next.item.type === 'text') {
      const prevText = prev.item as FlowTextItem;
      const nextText = next.item as FlowTextItem;
      return prevText.content === nextText.content &&
             prevText.isStreaming === nextText.isStreaming;
    }
    
    // Compare tool results and streaming params for tool items
    if (prev.item.type === 'tool' && next.item.type === 'tool') {
      const prevTool = prev.item as FlowToolItem;
      const nextTool = next.item as FlowToolItem;
      // Compare streaming params to re-render when they update
      return prevTool.toolResult === nextTool.toolResult &&
             prevTool.interruptionReason === nextTool.interruptionReason &&
             prevTool.acpPermission === nextTool.acpPermission &&
             prevTool.isParamsStreaming === nextTool.isParamsStreaming &&
             JSON.stringify(prevTool.partialParams) === JSON.stringify(nextTool.partialParams);
    }
    
    return true;
  }
);

FlowItemRenderer.displayName = 'FlowItemRenderer';
