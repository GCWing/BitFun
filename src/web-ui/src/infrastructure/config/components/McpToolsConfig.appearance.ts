import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const mcpToolsConfigAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'mcp-tools-config',
  parts: [
    { id: 'root' },
    { id: 'content' },
    { id: 'statusBadge' },
    { id: 'serverDetails' },
    { id: 'detailItem' },
    { id: 'detailLabel' },
    { id: 'detailValue' },
    { id: 'empty' },
    { id: 'jsonEditor' },
    { id: 'jsonHeader' },
    { id: 'jsonHint' },
    { id: 'jsonTextarea' },
    { id: 'jsonActions' },
    { id: 'examples' },
    { id: 'example' },
    { id: 'authEditor' },
    { id: 'authorizationInput' },
  ],
  facets: [
    { id: 'view', attribute: 'data-openbitfun-view', values: ['list', 'json'] },
  ],
};
