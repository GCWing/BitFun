import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const flowTextBlockAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'flow-text-block',
  parts: [
    { id: 'root' },
    { id: 'textContent' },
    { id: 'protocol' },
    { id: 'protocolTextContent' },
  ],
  facets: [{ id: 'mode', attribute: 'data-bf-mode', values: ['markdown', 'text'] }],
  states: [{ id: 'streaming', selector: { kind: 'self', suffix: '[data-bf-state~="streaming"]' } }],
};
