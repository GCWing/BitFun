import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const branchesViewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'branches-view',
  parts: [
    { id: 'root' }, { id: 'placeholder' }, { id: 'left' }, { id: 'toolbar' }, { id: 'search' },
    { id: 'actions' }, { id: 'list' }, { id: 'empty' }, { id: 'branch' },
    { id: 'branchInfo' }, { id: 'branchActions' }, { id: 'right' },
    { id: 'historyToolbar' }, { id: 'historyTitle' }, { id: 'historyList' },
    { id: 'commit' }, { id: 'commitHeader' }, { id: 'commitInfo' },
    { id: 'commitActions' }, { id: 'commitDetails' },
  ],
  states: [
    { id: 'current', selector: { kind: 'self', suffix: '[data-openbitfun-state~="current"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-openbitfun-state~="selected"]' } },
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-openbitfun-state~="expanded"]' } },
  ],
};
