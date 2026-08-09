import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const acpAgentsConfigAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'acp-agents-config',
  parts: [
    { id: 'root' },
    { id: 'content' },
    { id: 'manager' },
    { id: 'toolbar' },
    { id: 'jsonEditor' },
    { id: 'jsonActions' },
    { id: 'empty' },
    { id: 'registryList' },
    { id: 'registryRow' },
    { id: 'registryMain' },
    { id: 'capabilities' },
    { id: 'status' },
    { id: 'confirmation' },
    { id: 'remoteList' },
    { id: 'remoteServer' },
    { id: 'remoteHeader' },
    { id: 'remoteAgents' },
  ],
  facets: [
    { id: 'view', attribute: 'data-bf-view', values: ['registry', 'json'] },
  ],
};
