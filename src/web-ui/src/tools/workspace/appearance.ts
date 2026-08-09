import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const workspaceToolAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'workspace-tool',
  parts: [
    { id: 'root' },
    { id: 'currentSection' },
    { id: 'recentSection' },
    { id: 'currentCard' },
    { id: 'recentCard' },
    { id: 'assistantCard' },
    { id: 'empty' },
    { id: 'error' },
  ],
};
