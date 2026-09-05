import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const taskDetailPanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'task-detail-panel',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'content' }, { id: 'empty' },
    { id: 'errorBanner' }, { id: 'reviewer' }, { id: 'prompt' },
    { id: 'actions' }, { id: 'error' }, { id: 'execution' }, { id: 'loading' },
  ],
  states: [
    { id: 'empty', selector: { kind: 'self', suffix: '[data-openbitfun-state~="empty"]' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
  ],
};
