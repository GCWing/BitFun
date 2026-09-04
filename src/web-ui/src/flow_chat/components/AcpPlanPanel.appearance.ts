import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const acpPlanPanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'acp-plan-panel',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'title' }, { id: 'progress' },
    { id: 'list' }, { id: 'item' }, { id: 'content' },
  ],
  facets: [{ id: 'status', attribute: 'data-openbitfun-status', values: ['completed', 'in_progress', 'pending'] }],
};
