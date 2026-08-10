import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const getFileDiffDisplayAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'get-file-diff-display',
  parts: [
    { id: 'root' }, { id: 'info' }, { id: 'fileName' }, { id: 'diffType' },
    { id: 'stats' }, { id: 'expanded' }, { id: 'message' }, { id: 'preview' },
    { id: 'error' },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
  ],
};
