import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const newProjectDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'new-project-dialog',
  parts: [
    { id: 'root' }, { id: 'hero' }, { id: 'title' }, { id: 'content' },
    { id: 'field' }, { id: 'pathSelector' }, { id: 'preview' },
    { id: 'error' }, { id: 'footer' },
  ],
};
