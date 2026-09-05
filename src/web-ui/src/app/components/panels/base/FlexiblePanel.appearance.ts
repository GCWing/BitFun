import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const flexiblePanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'flexible-panel',
  parts: [
    { id: 'root' },
    { id: 'header' },
    { id: 'headerMain' },
    { id: 'headerActions' },
    { id: 'content' },
    { id: 'loading' },
    { id: 'empty' },
    { id: 'code' },
    { id: 'markdown' },
    { id: 'markdownEditor' },
    { id: 'text' },
    { id: 'viewer' },
    { id: 'image' },
    { id: 'pdf' },
    { id: 'error' },
    { id: 'aiSession' },
    { id: 'sessionHeader' },
    { id: 'sessionDetails' },
    { id: 'operations' },
    { id: 'operation' },
    { id: 'terminal' },
    { id: 'unknown' },
  ],
  states: [
    { id: 'needsFix', selector: { kind: 'self', suffix: '[data-openbitfun-state~="needsFix"]' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
  ],
};
