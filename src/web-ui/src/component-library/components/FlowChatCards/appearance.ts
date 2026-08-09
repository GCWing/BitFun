import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const flowChatCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'flow-chat-card',
  parts: [
    { id: 'root' }, { id: 'compact' }, { id: 'header' }, { id: 'content' },
    { id: 'status' }, { id: 'actions' }, { id: 'result' }, { id: 'error' },
    { id: 'details' }, { id: 'list' }, { id: 'item' }, { id: 'empty' },
    { id: 'processing' }, { id: 'processingDot' },
  ],
  facets: [
    { id: 'display', attribute: 'data-bf-display', values: ['compact', 'normal', 'detailed'] },
    { id: 'status', attribute: 'data-bf-status', values: ['pending', 'preparing', 'running', 'streaming', 'completed', 'cancelled', 'error'] },
  ],
};
