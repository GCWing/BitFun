import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const turnCompletionNoticeAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'turn-completion-notice',
  parts: [
    { id: 'root' }, { id: 'icon' }, { id: 'content' }, { id: 'text' },
    { id: 'title' }, { id: 'body' }, { id: 'reason' }, { id: 'reasonCode' },
  ],
  facets: [{ id: 'tone', attribute: 'data-openbitfun-tone', values: ['error', 'info', 'warning'] }],
};
