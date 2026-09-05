import React from 'react';
import { Icon, IconButton, Tooltip } from '@openbitfun/ui';

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
        icon={<Icon name="spark" size="md" />}
      />
    </Tooltip>
  );
};

export default MiniAppCustomizeEntry;
