import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const inputDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'input-dialog',
  parts: [{ id: 'root' }, { id: 'body' }, { id: 'description' }, { id: 'error' }, { id: 'actions' }],
};
