import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const flowTextBlockAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'flow-text-block',
  parts: [
    { id: 'root' },
    { id: 'textContent' },
    { id: 'protocol' },
    { id: 'protocolTextContent' },
  ],
  facets: [{ id: 'mode', attribute: 'data-openbitfun-mode', values: ['markdown', 'text'] }],
  states: [{ id: 'streaming', selector: { kind: 'self', suffix: '[data-openbitfun-state~="streaming"]' } }],
};
