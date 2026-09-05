import React, { useRef } from 'react';
import {
  MobileActionSheet,
  MobileButton,
  MobileConfirmSheet,
  MobileSheet,
  MobileTextField,
} from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';
import type { SessionInfo } from '../services/RemoteSessionManager';
import HarnessProfilePicker from './HarnessProfilePicker';

interface SessionOverlaysProps {
  compact: boolean;
  deleteTarget: SessionInfo | null;
  deleting: boolean;
  harnessOpen: boolean;
  menuSession: SessionInfo | null;
  onCloseDelete: () => void;
  onCloseDisconnect: () => void;
  onCloseHarness: () => void;
  onCloseMenu: () => void;
  onCloseRename: () => void;
  onConfirmDelete: () => void;
  onConfirmDisconnect: () => void;
  onConfirmRename: () => void;
  onDeleteRequest: (session: SessionInfo) => void;
  onHarnessSelect: (agentType: string) => void;
  onRenameRequest: (session: SessionInfo) => void;
  onRenameValueChange: (value: string) => void;
  renameTarget: SessionInfo | null;
  renameValue: string;
  renaming: boolean;
  showDisconnectConfirm: boolean;
}

const RenameIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
    <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
  </svg>
);

const DeleteIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <polyline points="3 6 5 6 21 6" />
    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
  </svg>
);

const WarningIcon = () => (
  <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <circle cx="12" cy="12" r="10" />
    <line x1="12" y1="8" x2="12" y2="12" />
    <line x1="12" y1="16" x2="12.01" y2="16" />
  </svg>
);

const DisconnectIcon = () => (
  <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
    <polyline points="16 17 21 12 16 7" />
    <line x1="21" y1="12" x2="9" y2="12" />
  </svg>
);

function RenameSessionSheet({ compact, onClose, onConfirm, onValueChange, open, pending, value }: {
  compact: boolean;
  onClose: () => void;
  onConfirm: () => void;
  onValueChange: (value: string) => void;
  open: boolean;
  pending: boolean;
  value: string;
}) {
  const { t } = useI18n();
  const compositionActiveRef = useRef(false);

  return (
    <MobileSheet
      className="session-list__rename-modal"
      footer={(
        <div className="session-list__rename-actions">
          <MobileButton className="session-list__rename-btn session-list__rename-btn--cancel" disabled={pending} onClick={onClose}>
            {t('sessions.cancel')}
          </MobileButton>
          <MobileButton appearance="primary" className="session-list__rename-btn session-list__rename-btn--save" disabled={pending || !value.trim()} loading={pending} onClick={onConfirm}>
            {t('sessions.save')}
          </MobileButton>
        </div>
      )}
      onOpenChange={() => !pending && onClose()}
      open={open}
      showHandle={compact}
      title={t('sessions.renameTitle')}
    >
      <MobileTextField
        appearance="surface"
        autoFocus
        className="session-list__rename-input"
        onChange={(event) => onValueChange(event.target.value)}
        onCompositionEnd={() => { compositionActiveRef.current = false; }}
        onCompositionStart={() => { compositionActiveRef.current = true; }}
        onKeyDown={(event) => {
          const nativeEvent = event.nativeEvent as KeyboardEvent;
          const imeOwned = compositionActiveRef.current || nativeEvent.isComposing || nativeEvent.keyCode === 229;
          if ((event.key === 'Enter' || event.key === 'Escape') && imeOwned) {
            event.stopPropagation();
            return;
          }
          if (event.key === 'Enter' && value.trim() && !pending) onConfirm();
          if (event.key === 'Escape' && !pending) onClose();
        }}
        placeholder={t('sessions.sessionNamePlaceholder')}
        type="text"
        value={value}
      />
    </MobileSheet>
  );
}

export default function SessionOverlays({
  compact,
  deleteTarget,
  deleting,
  harnessOpen,
  menuSession,
  onCloseDelete,
  onCloseDisconnect,
  onCloseHarness,
  onCloseMenu,
  onCloseRename,
  onConfirmDelete,
  onConfirmDisconnect,
  onConfirmRename,
  onDeleteRequest,
  onHarnessSelect,
  onRenameRequest,
  onRenameValueChange,
  renameTarget,
  renameValue,
  renaming,
  showDisconnectConfirm,
}: SessionOverlaysProps) {
  const { t } = useI18n();

  return (
    <>
      <MobileActionSheet
        actions={menuSession ? [
          { id: 'rename', label: t('sessions.renameSession'), leading: <RenameIcon /> },
          { id: 'delete', label: t('sessions.deleteSession'), leading: <DeleteIcon />, tone: 'danger' },
        ] : []}
        cancelLabel={t('sessions.cancel')}
        onAction={(id) => {
          if (!menuSession) return;
          if (id === 'rename') onRenameRequest(menuSession);
          if (id === 'delete') onDeleteRequest(menuSession);
        }}
        onOpenChange={onCloseMenu}
        open={menuSession !== null && renameTarget === null && deleteTarget === null}
        title={menuSession?.name || t('sessions.untitledSession')}
      />

      <RenameSessionSheet compact={compact} onClose={onCloseRename} onConfirm={onConfirmRename} onValueChange={onRenameValueChange} open={renameTarget !== null} pending={renaming} value={renameValue} />

      <MobileConfirmSheet
        cancelLabel={t('sessions.cancel')}
        confirmLabel={t('sessions.deleteSession')}
        confirmTone="danger"
        description={deleteTarget ? <><strong>“{deleteTarget.name || t('sessions.untitledSession')}”</strong><br />{t('sessions.confirmDeleteDesc')}</> : undefined}
        icon={<WarningIcon />}
        onConfirm={onConfirmDelete}
        onOpenChange={onCloseDelete}
        open={deleteTarget !== null}
        pending={deleting}
        showHandle={compact}
        title={t('sessions.confirmDelete')}
      />

      <MobileConfirmSheet
        cancelLabel={t('common.cancel')}
        confirmLabel={t('sessions.disconnect')}
        confirmTone="danger"
        description={t('sessions.disconnectConfirm')}
        icon={<DisconnectIcon />}
        onConfirm={onConfirmDisconnect}
        onOpenChange={onCloseDisconnect}
        open={showDisconnectConfirm}
        showHandle={compact}
        title={t('sessions.disconnect')}
      />

      <HarnessProfilePicker open={harnessOpen} onClose={onCloseHarness} onSelect={onHarnessSelect} />
    </>
  );
}
