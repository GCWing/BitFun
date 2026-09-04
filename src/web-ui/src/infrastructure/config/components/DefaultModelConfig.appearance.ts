import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const defaultModelConfigAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'default-model-config',
  parts: [
    { id: 'root' }, { id: 'loading' }, { id: 'empty' },
    { id: 'primaryModel' }, { id: 'lightweightModel' }, { id: 'embeddingModel' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
    { id: 'empty', selector: { kind: 'self', suffix: '[data-openbitfun-state~="empty"]' } },
  ],
};
