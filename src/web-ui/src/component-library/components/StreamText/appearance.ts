import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const streamTextAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'stream-text',
  parts: [{ id: 'root' }, { id: 'content' }, { id: 'character' }, { id: 'cursor' }],
  facets: [
    { id: 'effect', attribute: 'data-bf-effect', values: ['smooth', 'typewriter', 'wave', 'fade', 'glitch', 'neon', 'matrix', 'gradient', 'pulse', 'blur', 'bounce', 'shimmer'] },
    { id: 'color', attribute: 'data-bf-color', values: ['blue', 'purple', 'green', 'rainbow', 'fire', 'ocean', 'sunset'] },
  ],
  states: [
    { id: 'streaming', selector: { kind: 'self', suffix: '[data-bf-state~="streaming"]' } },
    { id: 'complete', selector: { kind: 'self', suffix: '[data-bf-state~="complete"]' } },
  ],
};
