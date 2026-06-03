import React from 'react';
import { Modal } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import ScheduledJobsView from '@/app/components/scheduled-jobs/ScheduledJobsView';
import type { CronJobTargetKind } from '@/infrastructure/api';
import type { WorkspaceKind } from '@/shared/types';
import './ScheduledJobsModal.scss';

interface ScheduledJobsModalProps {
  isOpen: boolean;
  onClose: () => void;
  workspacePath?: string;
  workspaceId?: string;
  workspaceKind?: WorkspaceKind;
  remoteConnectionId?: string | null;
  remoteSshHost?: string | null;
  sessionId?: string;
  targetKind: CronJobTargetKind;
  lockSessionId?: boolean;
  title?: string;
  targetLabel?: string;
  targetDescription?: string;
}

const ScheduledJobsModal: React.FC<ScheduledJobsModalProps> = ({
  isOpen,
  onClose,
  workspacePath,
  workspaceId,
  workspaceKind,
  remoteConnectionId,
  remoteSshHost,
  sessionId,
  targetKind,
  lockSessionId = false,
  title,
  targetLabel,
  targetDescription,
}) => {
  const { t } = useI18n('common');

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={title || t('nav.scheduledJobs.title')}
      size="xlarge"
    >
      <div className="scheduled-jobs-modal__body">
        <ScheduledJobsView
          workspacePath={workspacePath}
          workspaceId={workspaceId}
          workspaceKind={workspaceKind}
          remoteConnectionId={remoteConnectionId}
          remoteSshHost={remoteSshHost}
          sessionId={sessionId}
          targetKind={targetKind}
          lockSessionId={lockSessionId}
          headerTitle={null}
          targetLabel={targetLabel}
          targetDescription={targetDescription}
        />
      </div>
    </Modal>
  );
};

export default ScheduledJobsModal;
