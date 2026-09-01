/**
 * The session usage report, as a dismissable overlay.
 *
 * Mounted once at the app root, like `ConfirmDialogRenderer`, and driven from
 * `sessionUsageModalState` — the request that produces a report runs several
 * layers below any component, and the report is not owned by whichever session
 * view happens to be mounted.
 *
 * The card inside is the same `SessionUsageReportCard` the transcript renders
 * for reports written by earlier versions, including its own waiting state.
 */

import React, { useCallback, useEffect, useSyncExternalStore } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@bitfun/ui';
import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';
import { setChatPopupActive } from '../chatPopupState';
import { SessionUsageReportCard } from './SessionUsageReportCard';
import type { SessionUsagePanelTab } from './sessionUsagePanelTypes';
import {
  closeSessionUsageModal,
  getSessionUsageModalState,
  subscribeSessionUsageModal,
} from './sessionUsageModalState';
import './SessionUsageModal.scss';

export const SessionUsageModal: React.FC = () => {
  const { t } = useTranslation('flow-chat');
  const state = useSyncExternalStore(subscribeSessionUsageModal, getSessionUsageModalState);
  const { open, isLoading, report, markdown, sessionId, workspacePath } = state;

  /*
   * The same latch the slash-command and @-mention popups use. Without it the
   * container's global Escape shortcut fires alongside the modal's own, and
   * dismissing a report would cancel whatever the session is running.
   */
  useEffect(() => {
    if (!open) return undefined;
    setChatPopupActive(true);
    return () => setChatPopupActive(false);
  }, [open]);

  const handleOpenDetails = useCallback((
    detailReport: SessionUsageReport,
    initialTab?: SessionUsagePanelTab,
  ) => {
    void import('../../services/openSessionUsageReport').then(({ openSessionUsagePanel }) => {
      openSessionUsagePanel({
        report: detailReport,
        markdown,
        sessionId,
        workspacePath,
        initialTab,
        title: t('usage.title'),
        expand: true,
      });
      // The panel is the fuller answer to the same question, so the summary
      // stops competing with it for the screen.
      closeSessionUsageModal();
    });
  }, [markdown, sessionId, t, workspacePath]);

  if (!open) return null;

  return (
    <Dialog
      open
      onOpenChange={(nextOpen) => { if (!nextOpen) closeSessionUsageModal(); }}
      size="xl"
      className="session-usage-dialog"
      data-testid="session-usage-modal"
    >
      <DialogHeader className="session-usage-dialog__header">
        <DialogHeading className="session-usage-dialog__heading">
          <DialogTitle className="session-usage-dialog__title">{t('usage.title')}</DialogTitle>
        </DialogHeading>
        <DialogClose data-testid="session-usage-modal-close" />
      </DialogHeader>
      <DialogBody className="session-usage-dialog__body" inset="none">
        <div
          className="session-usage-dialog__content"
          data-bf-component="session-usage-modal"
          data-bf-part="content"
        >
          <SessionUsageReportCard
            compact
            report={report}
            markdown={markdown}
            generatedAt={report?.generatedAt}
            isLoading={isLoading}
            onOpenDetails={report ? handleOpenDetails : undefined}
          />
        </div>
      </DialogBody>
    </Dialog>
  );
};

SessionUsageModal.displayName = 'SessionUsageModal';

export default SessionUsageModal;
