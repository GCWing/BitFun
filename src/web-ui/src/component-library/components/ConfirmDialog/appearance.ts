import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const confirmDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'confirm-dialog',
  parts: [
    { id: 'root' }, { id: 'icon' }, { id: 'content' }, { id: 'title' },
    { id: 'message' }, { id: 'preview' }, { id: 'actions' },
  ],
  facets: [{ id: 'type', attribute: 'data-bf-type', values: ['info', 'warning', 'error', 'success'] }],
};
