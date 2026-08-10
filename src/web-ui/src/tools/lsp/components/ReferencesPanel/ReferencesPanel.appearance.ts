import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const referencesPanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'references-panel',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'title' }, { id: 'close' },
    { id: 'empty' }, { id: 'content' }, { id: 'fileGroup' }, { id: 'fileHeader' },
    { id: 'filePath' }, { id: 'fileCount' }, { id: 'list' }, { id: 'item' },
    { id: 'line' }, { id: 'preview' },
  ],
  states: [
    { id: 'empty', selector: { kind: 'self', suffix: '[data-bf-state~="empty"]' } },
  ],
};
