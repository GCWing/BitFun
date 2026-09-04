import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const mcpResourceBrowserAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'mcp-resource-browser',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'headerActions' }, { id: 'search' },
    { id: 'content' }, { id: 'list' }, { id: 'loading' }, { id: 'empty' },
    { id: 'resource' }, { id: 'resourceIcon' }, { id: 'resourceInfo' },
    { id: 'viewer' }, { id: 'viewerHeader' }, { id: 'viewerContent' },
  ],
  states: [{ id: 'selected', selector: { kind: 'self', suffix: '[data-openbitfun-state~="selected"]' } }],
};
