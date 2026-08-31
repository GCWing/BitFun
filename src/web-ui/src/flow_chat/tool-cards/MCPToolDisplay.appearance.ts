import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const mcpToolDisplayAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'mcp-tool-display',
  parts: [
    { id: 'root' }, { id: 'info' }, { id: 'expanded' },
    { id: 'item' }, { id: 'input' }, { id: 'text' }, { id: 'image' }, { id: 'resource' },
    { id: 'iframe' }, { id: 'loading' }, { id: 'error' },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
  ],
};
