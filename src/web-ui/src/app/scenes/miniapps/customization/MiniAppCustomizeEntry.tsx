import React from 'react';
import { IconButton, Tooltip } from '@bitfun/ui';
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
    <Tooltip content={label} disabled={disabled}>
      <IconButton
        size="sm"
        shape="square"
        onClick={onOpen}
        disabled={disabled}
        aria-label={label}
        icon={<WandSparkles />}
      />
    </Tooltip>
  );
};

export default MiniAppCustomizeEntry;
