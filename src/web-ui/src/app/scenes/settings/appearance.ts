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
    attribute: 'data-openbitfun-page',
    values: [
      'application.general',
      'application.appearance',
      'application.pet',
      'application.voice',
      'application.shortcuts',
      'application.terminal',
      'application.editor',
      'ai.models',
      'ai.memory',
      'workspace.session',
      'workspace.worktrees',
      'tools.execution',
      'tools.desktop-control',
      'tools.automation',
      'tools.mcp',
      'tools.acp',
      'data.usage',
      'data.archived',
      'data.diagnostics',
    ],
  }],
};
