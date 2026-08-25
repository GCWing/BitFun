import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const imageAnalysisCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'image-analysis-card',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'thumbnail' }, { id: 'placeholder' },
    { id: 'info' }, { id: 'filename' }, { id: 'status' }, { id: 'content' },
    { id: 'summary' }, { id: 'details' }, { id: 'section' },
    { id: 'tags' }, { id: 'metadata' }, { id: 'error' },
  ],
  facets: [{ id: 'status', attribute: 'data-bf-status', values: ['analyzing', 'completed', 'error'] }],
  states: [{ id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } }],
};
