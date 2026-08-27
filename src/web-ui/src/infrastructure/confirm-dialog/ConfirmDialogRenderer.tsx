import { ConfirmDialog } from '@bitfun/ui';
import { useI18n } from '@/infrastructure/i18n';
import { useConfirmDialogStore } from './confirmDialogService';

export function ConfirmDialogRenderer() {
  const { t } = useI18n('components');
  const { cancel, confirm, isOpen, options, secondary } = useConfirmDialogStore();

  if (!options) return null;

  return (
    <ConfirmDialog
      cancelText={options.cancelText ?? t('dialog.confirm.cancel')}
      confirmDanger={options.confirmDanger}
      confirmText={options.confirmText ?? t('dialog.confirm.ok')}
      isOpen={isOpen}
      message={options.message}
      onClose={cancel}
      onConfirm={confirm}
      onSecondary={secondary}
      preview={options.preview}
      previewMaxHeight={options.previewMaxHeight}
      secondaryText={options.secondaryText}
      showCancel={options.showCancel}
      title={options.title}
      type={options.type}
    />
  );
}
