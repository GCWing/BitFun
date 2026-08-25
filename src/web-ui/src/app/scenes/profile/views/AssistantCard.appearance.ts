import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const assistantCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'assistant-card',
  parts: [
    { id: 'root' }, { id: 'main' }, { id: 'header' }, { id: 'avatar' },
    { id: 'headerInfo' }, { id: 'name' }, { id: 'primaryBadge' }, { id: 'badges' },
    { id: 'vibe' }, { id: 'footer' }, { id: 'setPrimary' }, { id: 'delete' },
    { id: 'chevron' },
  ],
  facets: [{ id: 'primary', attribute: 'data-bf-primary', values: ['true', 'false'] }],
  states: [
    { id: 'busy', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="busy"]' } },
  ],
};
