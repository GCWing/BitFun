import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const gitDiffViewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'git-diff-view',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'headerLeft' }, { id: 'headerRight' },
    { id: 'content' }, { id: 'loading' }, { id: 'error' },
    { id: 'fileList' }, { id: 'file' }, { id: 'fileHeader' }, { id: 'fileInfo' },
    { id: 'fileStats' }, { id: 'diffContent' }, { id: 'diffLine' },
    { id: 'lineNumber' }, { id: 'lineContent' }, { id: 'empty' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
  ],
};
