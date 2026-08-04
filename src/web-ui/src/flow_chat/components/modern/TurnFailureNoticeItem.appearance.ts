import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const turnFailureNoticeAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'turn-failure-notice',
  parts: [
    { id: 'root' }, { id: 'icon' }, { id: 'content' }, { id: 'header' },
    { id: 'summary' }, { id: 'title' }, { id: 'message' }, { id: 'toggle' },
    { id: 'details' }, { id: 'facts' }, { id: 'fact' }, { id: 'rawError' },
    { id: 'rawHeader' }, { id: 'copy' }, { id: 'code' },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-bf-state~="open"]' } },
    { id: 'copied', selector: { kind: 'self', suffix: '[data-bf-state~="copied"]' } },
  ],
};
