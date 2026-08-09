import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const workspaceProjectPermissionsDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'workspace-project-permissions-dialog',
  parts: [
    { id: 'root' }, { id: 'intro' }, { id: 'section' }, { id: 'sectionHeader' },
    { id: 'grants' }, { id: 'grant' }, { id: 'rules' }, { id: 'rule' },
    { id: 'ruleActions' }, { id: 'empty' }, { id: 'footer' },
  ],
};
