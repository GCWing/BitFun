import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const globalPermissionRulesDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'global-permission-rules-dialog',
  parts: [
    { id: 'root' }, { id: 'intro' }, { id: 'section' }, { id: 'sectionHeader' },
    { id: 'empty' }, { id: 'rules' }, { id: 'rule' }, { id: 'ruleActions' },
    { id: 'footer' },
  ],
};
