import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const gitBranchHistoryAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'git-branch-history',
  parts: [
    { id: 'root' }, { id: 'loading' }, { id: 'error' }, { id: 'header' },
    { id: 'headerLeft' }, { id: 'headerRight' }, { id: 'search' }, { id: 'content' },
    { id: 'empty' }, { id: 'commits' }, { id: 'commit' }, { id: 'commitMain' },
    { id: 'timeline' }, { id: 'commitInfo' }, { id: 'commitDetails' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
  ],
};
