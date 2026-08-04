import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const contextCompressionDisplayAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'context-compression-display',
  parts: [
    { id: 'root' }, { id: 'info' }, { id: 'tokenStat' }, { id: 'savings' },
    { id: 'processing' }, { id: 'meta' }, { id: 'detail' }, { id: 'error' },
  ],
  states: [
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
    { id: 'fallback', selector: { kind: 'self', suffix: '[data-bf-state~="fallback"]' } },
  ],
};
