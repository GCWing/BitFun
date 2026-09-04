import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const acpPermissionActionsAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'acp-permission-actions',
  parts: [{ id: 'root' }, { id: 'action' }],
  facets: [
    { id: 'presentation', attribute: 'data-openbitfun-presentation', values: ['icon', 'text'] },
    { id: 'decision', attribute: 'data-openbitfun-decision', values: ['allow', 'reject'] },
  ],
  states: [{ id: 'disabled', selector: { kind: 'self', suffix: ':disabled' } }],
};
