import { IconButton } from '@bitfun/ui';
import React from 'react';
import { Check, Copy } from 'lucide-react';
import { useCopyTextAction } from '../hooks/useCopyTextAction';
import './ToolCardHeaderActions.scss';

interface ToolCardHeaderActionsProps {
  children: React.ReactNode;
  className?: string;
}

export const ToolCardHeaderActions: React.FC<ToolCardHeaderActionsProps> = ({
  children,
  className,
}) => (
  <span
    className={`tool-card-header-actions${className ? ` ${className}` : ''}`}
    onClick={(event) => event.stopPropagation()}
    data-bf-component="tool-card-header-actions"
    data-bf-part="root"
  >
    {children}
  </span>
);

interface ToolCardCopyActionProps {
  getText: () => string;
  tooltip: string;
  copiedTooltip?: string;
  successMessage: string;
  failureMessage: string;
  ariaLabel?: string;
  className?: string;
  disabled?: boolean;
  showSuccessNotification?: boolean;
}

export const ToolCardCopyAction: React.FC<ToolCardCopyActionProps> = ({
  getText,
  tooltip,
  copiedTooltip,
  successMessage,
  failureMessage,
  ariaLabel,
  className,
  disabled,
  showSuccessNotification,
}) => {
  const { copied, copy } = useCopyTextAction({
    getText,
    successMessage,
    failureMessage,
    showSuccessNotification,
  });

  return (
    <IconButton
      data-bf-component="tool-card-header-actions"
      data-bf-part="action"
      className={`tool-card-header-action tool-card-copy-action${copied ? ' copied' : ''}${className ? ` ${className}` : ''}`}
      size="sm"
      onClick={copy}
      disabled={disabled}
      title={copied ? (copiedTooltip ?? successMessage) : tooltip}
      aria-label={ariaLabel ?? tooltip}
    >
      {copied ? <Check size={12} /> : <Copy size={12} />}
    </IconButton>
  );
};
