import { IconButton } from '@bitfun/ui';
import React from 'react';
import { WandSparkles } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';

interface MiniAppCustomizeEntryProps {
  disabled?: boolean;
  onOpen: () => void;
}

export const MiniAppCustomizeEntry: React.FC<MiniAppCustomizeEntryProps> = ({
  disabled,
  onOpen,
}) => {
  const { t } = useI18n('scenes/miniapp');
  const label = t('customize.trigger');

  return (
    <IconButton
      size="md"
      onClick={onOpen}
      disabled={disabled}
      title={label}
      aria-label={label}
    >
      <WandSparkles size={14} />
    </IconButton>
  );
};

export default MiniAppCustomizeEntry;
