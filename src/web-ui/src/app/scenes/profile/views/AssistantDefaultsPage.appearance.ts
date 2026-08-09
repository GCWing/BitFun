import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const assistantDefaultsPageAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'assistant-defaults-page',
  parts: [
    { id: 'root' }, { id: 'toolbar' }, { id: 'content' }, { id: 'shell' },
    { id: 'main' }, { id: 'header' }, { id: 'zones' }, { id: 'split' },
    { id: 'skillList' }, { id: 'skill' }, { id: 'toolList' }, { id: 'tool' },
    { id: 'group' }, { id: 'groupHeader' }, { id: 'detail' },
    { id: 'detailHeader' }, { id: 'detailBody' }, { id: 'empty' }, { id: 'loading' },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
    { id: 'disabled', selector: { kind: 'self', suffix: '[data-bf-state~="disabled"]' } },
    { id: 'covered', selector: { kind: 'self', suffix: '[data-bf-state~="covered"]' } },
    { id: 'collapsed', selector: { kind: 'self', suffix: '[data-bf-state~="collapsed"]' } },
  ],
};
