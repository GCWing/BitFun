import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const automationSettingsPageAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'automation-settings-page',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'content' },
    { id: 'quickActions', visualRole: 'content' },
    { id: 'hooks', visualRole: 'content' },
  ],
};

export const developmentSettingsPageAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'development-settings-page',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'content' },
    { id: 'terminal', visualRole: 'content' },
    { id: 'editor', visualRole: 'content' },
  ],
};

export const inputSettingsPageAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'input-settings-page',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'content' },
    { id: 'voice', visualRole: 'content' },
    { id: 'shortcuts', visualRole: 'content' },
  ],
};
