import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const exportImageAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'export-image',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'logo' }, { id: 'title' },
    { id: 'timestamp' }, { id: 'user' }, { id: 'assistant' }, { id: 'round' },
    { id: 'text' }, { id: 'thinking' }, { id: 'tool' }, { id: 'footer' },
    { id: 'trigger' },
  ],
  states: [{ id: 'exporting', selector: { kind: 'self', suffix: '[data-openbitfun-state~="exporting"]' } }],
};
