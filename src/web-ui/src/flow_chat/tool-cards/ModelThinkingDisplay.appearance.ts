import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const modelThinkingDisplayAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'model-thinking-display',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'leadingIcon' }, { id: 'label' },
    { id: 'expandContainer' }, { id: 'contentWrapper' }, { id: 'content' },
  ],
  facets: [{ id: 'context', attribute: 'data-bf-context', values: ['default', 'subagent-projection'] }],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'streaming', selector: { kind: 'self', suffix: '[data-bf-state~="streaming"]' } },
  ],
};
