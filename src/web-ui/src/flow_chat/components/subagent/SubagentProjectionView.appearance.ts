import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const subagentProjectionAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'subagent-projection',
  parts: [
    { id: 'root' }, { id: 'truncated' }, { id: 'hint' }, { id: 'message' },
    { id: 'item' }, { id: 'collapse' },
    { id: 'container' }, { id: 'content' },
  ],
  states: [
    { id: 'collapsed', selector: { kind: 'self', suffix: '[data-openbitfun-state~="collapsed"]' } },
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-openbitfun-state~="expanded"]' } },
  ],
};
