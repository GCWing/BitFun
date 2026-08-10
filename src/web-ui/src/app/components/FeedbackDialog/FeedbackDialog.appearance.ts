import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const feedbackDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'feedback-dialog',
  parts: [
    { id: 'root', visualRole: 'dialog' },
    { id: 'viewSwitch', visualRole: 'toolbar' },
    { id: 'form', visualRole: 'content' },
    { id: 'actions', visualRole: 'toolbar' },
  ],
};
