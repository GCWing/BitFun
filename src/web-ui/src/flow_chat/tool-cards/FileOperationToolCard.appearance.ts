import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const fileOperationToolCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'file-operation-tool-card',
  parts: [
    { id: 'root' },
    { id: 'preview' },
    { id: 'previewText' },
    { id: 'guidance' },
    { id: 'error' },
    { id: 'action' },
    { id: 'path' },
  ],
  facets: [
    { id: 'action', attribute: 'data-bf-action', values: ['create', 'modify', 'delete'] },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
  ],
};
