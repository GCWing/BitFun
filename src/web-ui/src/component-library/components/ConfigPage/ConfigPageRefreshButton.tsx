import React from 'react';
import { RefreshCw } from 'lucide-react';
import { IconButton } from '../IconButton';

export interface ConfigPageRefreshButtonProps {
  tooltip: React.ReactNode;
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
    <IconButton
      variant="ghost"
      size="small"
      tooltip={tooltip}
      onClick={onClick}
      disabled={disabled}
      isLoading={loading}
      className={className}
    >
      <RefreshCw size={14} />
    </IconButton>
  );
};

