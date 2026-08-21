import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const settingsAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'settings',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'workspace', continuityGroup: 'settings-workspace' },
    { id: 'content', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'settings-workspace' },
    { id: 'loading', propertyProfile: 'overlay', visualRole: 'content' },
  ],
  facets: [{
    id: 'page',
    attribute: 'data-bf-page',
    values: [
      'application.general',
      'application.appearance',
      'application.pet',
      'application.input',
      'application.development',
      'ai.models',
      'ai.memory',
      'workspace.session',
      'workspace.worktrees',
      'tools.execution',
      'tools.automation',
      'tools.mcp',
      'tools.acp',
      'data.usage',
      'data.archived',
      'data.diagnostics',
    ],
  }],
};
