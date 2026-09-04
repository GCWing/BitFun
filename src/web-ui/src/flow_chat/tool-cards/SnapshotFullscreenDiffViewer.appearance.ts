import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const snapshotFullscreenDiffViewerAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'snapshot-fullscreen-diff-viewer',
  parts: [
    { id: 'overlay' }, { id: 'root' }, { id: 'header' }, { id: 'sessionInfo' },
    { id: 'headerActions' }, { id: 'navigation' }, { id: 'tabs' }, { id: 'tab' },
    { id: 'currentFile' }, { id: 'fileInfo' }, { id: 'fileActions' },
    { id: 'content' }, { id: 'loading' },
  ],
  states: [{ id: 'active', selector: { kind: 'self', suffix: '[data-openbitfun-state~="active"]' } }],
};
