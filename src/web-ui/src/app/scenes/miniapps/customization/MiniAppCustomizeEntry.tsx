import React from 'react';
import { WandSparkles } from 'lucide-react';
import { IconButton } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';

interface MiniAppCustomizeEntryProps {
  disabled?: boolean;
  hotspot?: boolean;
  onOpen: () => void;
}

export const MiniAppCustomizeEntry: React.FC<MiniAppCustomizeEntryProps> = ({
  disabled,
  hotspot = false,
  onOpen,
}) => {
  const { t } = useI18n('scenes/miniapp');
  const label = hotspot ? t('customize.hotspotLabel') : t('customize.trigger');

  return (
    <IconButton
      variant={hotspot ? 'ai' : 'ghost'}
      size={hotspot ? 'medium' : 'small'}
      shape={hotspot ? 'circle' : 'square'}
      className={hotspot ? 'miniapp-scene__customize-hotspot-button' : undefined}
      onClick={onOpen}
      disabled={disabled}
      tooltip={label}
      aria-label={label}
    >
      <WandSparkles size={hotspot ? 18 : 14} />
    </IconButton>
  );
};

export default MiniAppCustomizeEntry;
