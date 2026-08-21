import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const workflowClawCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'workflow-claw-card',
  parts: [
    { id: 'root' }, { id: 'main' }, { id: 'header' }, { id: 'avatar' },
    { id: 'headerInfo' }, { id: 'name' }, { id: 'badges' },
    { id: 'configure' }, { id: 'chevron' },
  ],
};
