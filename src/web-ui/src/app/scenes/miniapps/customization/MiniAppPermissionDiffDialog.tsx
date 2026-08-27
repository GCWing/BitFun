import { Button, Modal } from '@bitfun/ui';
import React from 'react';
import { AlertTriangle } from 'lucide-react';
import type { MiniAppPermissionDiff } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useI18n } from '@/infrastructure/i18n';

interface MiniAppPermissionDiffDialogProps {
  isOpen: boolean;
  diff: MiniAppPermissionDiff | null;
  applying?: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

function PermissionList({ title, items }: { title: string; items: string[] }) {
  if (items.length === 0) {
    return null;
  }

  return (
    <section className="miniapp-permission-dialog__section">
      <h4>{title}</h4>
      <ul>
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </section>
  );
}

export const MiniAppPermissionDiffDialog: React.FC<MiniAppPermissionDiffDialogProps> = ({
  isOpen,
  diff,
  applying = false,
  onCancel,
  onConfirm,
}) => {
  const { t } = useI18n('scenes/miniapp');

  return (
    <Modal
      isOpen={isOpen}
      onClose={applying ? () => undefined : onCancel}
      title={t('customize.permissionDialog.title')}
      size="medium"
      closeOnOverlayClick={!applying}
      contentPadding="lg"
      overlayClassName="miniapp-permission-dialog"
    >
      <div className="miniapp-permission-dialog__body">
        <div className="miniapp-permission-dialog__notice">
          <AlertTriangle size={20} />
          <p>{t('customize.permissionDialog.body')}</p>
        </div>
        <PermissionList title={t('customize.permissionDialog.added')} items={diff?.added ?? []} />
        <PermissionList title={t('customize.permissionDialog.expanded')} items={diff?.expanded ?? []} />
        <PermissionList title={t('customize.permissionDialog.removed')} items={diff?.removed ?? []} />
        <div className="miniapp-permission-dialog__actions">
          <Button variant="outline" size="sm" onClick={onCancel} disabled={applying}>
            {t('customize.permissionDialog.cancel')}
          </Button>
          <Button variant="fill" tone="danger" size="sm" onClick={onConfirm} loading={applying}>
            {t('customize.permissionDialog.confirm')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default MiniAppPermissionDiffDialog;
