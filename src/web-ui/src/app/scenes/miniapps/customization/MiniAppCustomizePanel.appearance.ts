import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const miniAppCustomizePanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'miniapp-customize-panel',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'notice' }, { id: 'noticeActions' },
    { id: 'request' }, { id: 'actions' }, { id: 'error' }, { id: 'status' },
    { id: 'chat' }, { id: 'chatLoading' }, { id: 'footer' }, { id: 'busy' },
  ],
  facets: [{
    id: 'stage',
    attribute: 'data-openbitfun-stage',
    values: ['idle', 'notice', 'drafting', 'preview', 'permission-review', 'applying'],
  }],
  states: [
    { id: 'busy', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="busy"]' } },
    { id: 'error', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="error"]' } },
    { id: 'previewOpen', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="preview-open"]' } },
  ],
};
