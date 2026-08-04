import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const externalMcpOverviewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'external-mcp-overview',
  parts: [
    { id: 'root' }, { id: 'summary' }, { id: 'statusBadge' }, { id: 'serverDetails' },
    { id: 'detailItem' }, { id: 'detailLabel' }, { id: 'detailValue' },
    { id: 'import' }, { id: 'importPlan' }, { id: 'importList' }, { id: 'importOption' },
    { id: 'importOptionContent' }, { id: 'importOptionMeta' }, { id: 'importActions' }, { id: 'empty' },
  ],
  states: [
    { id: 'pending', selector: { kind: 'self', suffix: '[data-bf-state~="pending"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
    { id: 'muted', selector: { kind: 'self', suffix: '[data-bf-state~="muted"]' } },
  ],
};
