import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const toolbarModeAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'toolbar-mode',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'headerLeft' }, { id: 'title' },
    { id: 'headerActions' }, { id: 'overflowTrigger' }, { id: 'overflowMenu' },
    { id: 'overflowItem' }, { id: 'collapsedActions' }, { id: 'sessionSurface' },
    { id: 'content' }, { id: 'stream' }, { id: 'tool' }, { id: 'toolName' },
    { id: 'toolSummary' }, { id: 'todo' }, { id: 'todoProgress' },
    { id: 'todoCurrent' }, { id: 'streamText' }, { id: 'controls' },
  ],
  facets: [
    { id: 'contentKind', attribute: 'data-openbitfun-content-kind', values: ['text', 'tool', 'todo'] },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="expanded"]' } },
    { id: 'processing', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="processing"]' } },
    { id: 'error', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="error"]' } },
    { id: 'confirm', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="confirm"]' } },
    { id: 'streaming', selector: { kind: 'self', suffix: '[data-openbitfun-state~="streaming"]' } },
    { id: 'open', selector: { kind: 'self', suffix: '[data-openbitfun-state~="open"]' } },
  ],
};
