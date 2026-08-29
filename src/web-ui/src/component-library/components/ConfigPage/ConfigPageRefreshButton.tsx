import React from 'react';
import { RefreshCw } from 'lucide-react';
import { IconButton, Tooltip } from '@bitfun/ui';

export interface ConfigPageRefreshButtonProps {
  tooltip: string;
  onClick: () => void;
  loading?: boolean;
  disabled?: boolean;
  className?: string;
}

export const ConfigPageRefreshButton: React.FC<ConfigPageRefreshButtonProps> = ({
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
        icon={<RefreshCw size={14} />}
      />
    </Tooltip>
  );
};

