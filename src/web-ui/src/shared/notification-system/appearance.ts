import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const notificationAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'notification',
  parts: [
    { id: 'container' },
    { id: 'item' },
    { id: 'itemIcon' },
    { id: 'itemContent' },
    { id: 'itemTitle' },
    { id: 'itemMessage' },
    { id: 'itemActions' },
    { id: 'itemClose' },
    { id: 'progressItem' },
    { id: 'progressIcon' },
    { id: 'progressBar' },
    { id: 'progressFill' },
    { id: 'loadingItem' },
    { id: 'centerRoot' },
    { id: 'centerHeader' },
    { id: 'centerClose' },
    { id: 'centerList' },
    { id: 'centerItem' },
  ],
  states: [
    { id: 'success', selector: { kind: 'self', suffix: '[data-openbitfun-state~="success"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
    { id: 'unread', selector: { kind: 'self', suffix: '[data-openbitfun-state~="unread"]' } },
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-openbitfun-state~="expanded"]' } },
  ],
};
