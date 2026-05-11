import { createTab } from '@/shared/utils/tabUtils';
import type { PanelContent } from '@/app/components/panels/base/types';
import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';
import type { SessionUsagePanelTab } from '../components/usage/sessionUsagePanelTypes';

export const SESSION_USAGE_PANEL_TYPE = 'session-usage' as const;

export interface SessionUsagePanelData {
  report: SessionUsageReport;
  markdown?: string;
  sessionId?: string;
  workspacePath?: string;
  title?: string;
  initialTab?: SessionUsagePanelTab;
}

export interface SessionUsagePanelMetadata {
  duplicateCheckKey: string;
  reportId: string;
  sessionId: string;
  contentRole: 'session-usage';
}

export const getSessionUsageDuplicateKey = (reportId: string) => `session-usage-${reportId}`;

export function buildSessionUsagePanelContent(params: SessionUsagePanelData): PanelContent {
  return {
    type: SESSION_USAGE_PANEL_TYPE,
    // Caller supplies the localized title so this panel helper does not eagerly
    // initialize the i18n singleton in message-rendering tests.
    title: params.title ?? 'Session Usage',
    data: params,
    metadata: {
      duplicateCheckKey: getSessionUsageDuplicateKey(params.report.reportId),
      reportId: params.report.reportId,
      sessionId: params.sessionId ?? params.report.sessionId,
      contentRole: 'session-usage',
    } satisfies SessionUsagePanelMetadata,
  };
}

export function openSessionUsagePanel(params: SessionUsagePanelData & { expand?: boolean }): void {
  const content = buildSessionUsagePanelContent(params);

  if (params.expand !== false && typeof window !== 'undefined') {
    window.dispatchEvent(new window.CustomEvent('expand-right-panel'));
  }

  createTab({
    type: content.type,
    title: content.title,
    data: content.data,
    metadata: content.metadata,
    checkDuplicate: true,
    duplicateCheckKey: content.metadata?.duplicateCheckKey,
    replaceExisting: !!params.initialTab,
    mode: 'agent',
  });
}
