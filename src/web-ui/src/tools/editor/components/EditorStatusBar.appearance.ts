import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const editorStatusBarAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'editor-status-bar',
  parts: [
    { id: 'root' }, { id: 'left' }, { id: 'right' }, { id: 'item' },
    { id: 'selection' }, { id: 'separator' },
  ],
};
