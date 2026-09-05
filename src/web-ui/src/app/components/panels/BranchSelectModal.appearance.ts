import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const branchSelectModalAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'branch-select-modal',
  parts: [
    { id: 'overlay' }, { id: 'root' }, { id: 'close' }, { id: 'header' },
    { id: 'content' }, { id: 'input' }, { id: 'error' }, { id: 'list' },
    { id: 'loading' }, { id: 'item' }, { id: 'itemName' }, { id: 'badge' },
    { id: 'empty' }, { id: 'footer' }, { id: 'options' },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-openbitfun-state~="selected"]' } },
    { id: 'current', selector: { kind: 'self', suffix: '[data-openbitfun-state~="current"]' } },
  ],
};
