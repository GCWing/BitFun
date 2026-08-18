import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const miniAppDetailModalAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'mini-app-detail-modal',
  parts: [
    { id: 'root' },
    { id: 'hero' },
    { id: 'iconStage' },
    { id: 'icon' },
    { id: 'summary' },
    { id: 'tags' },
    { id: 'highlights' },
    { id: 'highlight' },
    { id: 'footer' },
    { id: 'status' },
    { id: 'actions' },
  ],
  states: [
    { id: 'running', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="running"]' } },
    { id: 'customizing', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="customizing"]' } },
  ],
};
