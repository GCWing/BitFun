import React from 'react';

import { Icon as CatalogIcon, IconButton, Tooltip } from '@openbitfun/ui';

export interface ConfigRefreshButtonProps {
  tooltip: string;
  onClick: () => void;
  loading?: boolean;
  disabled?: boolean;
  className?: string;
}

export const ConfigRefreshButton: React.FC<ConfigRefreshButtonProps> = ({
  tooltip,
  onClick,
  loading = false,
  disabled = false,
  className = '',
}) => {
  return (
    <Tooltip content={tooltip} disabled={disabled}>
      <IconButton
        aria-label={tooltip}
        variant="quiet"
        size="sm"
        onClick={onClick}
        disabled={disabled}
        loading={loading}
        className={className}
        icon={<CatalogIcon name="refresh" size="sm" />}
      />
    </Tooltip>
  );
};

