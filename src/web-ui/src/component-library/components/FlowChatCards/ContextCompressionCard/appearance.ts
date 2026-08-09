import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const contextCompressionCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'context-compression-card',
  parts: [
    { id: 'compact' }, { id: 'status' }, { id: 'action' }, { id: 'tokens' },
    { id: 'result' }, { id: 'processing' }, { id: 'summaryRow' }, { id: 'statsRow' },
  ],
  facets: [
    { id: 'display', attribute: 'data-bf-display', values: ['compact'] },
    { id: 'status', attribute: 'data-bf-status', values: ['pending', 'preparing', 'running', 'streaming', 'completed', 'cancelled', 'error'] },
  ],
};
