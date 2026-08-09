import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const turnHistoryPanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'turn-history-panel',
  parts: [
    { id: 'root' }, { id: 'loading' }, { id: 'empty' }, { id: 'header' },
    { id: 'count' }, { id: 'list' }, { id: 'item' }, { id: 'itemHeader' },
    { id: 'files' }, { id: 'filesList' }, { id: 'time' },
  ],
  states: [{ id: 'current', selector: { kind: 'self', suffix: '[data-bf-state~="current"]' } }],
};
