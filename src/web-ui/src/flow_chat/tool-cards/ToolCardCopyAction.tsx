import { Check, Copy } from 'lucide-react';
import { IconButton } from '@bitfun/ui';
import { useCopyTextAction } from '../hooks/useCopyTextAction';

export interface ToolCardCopyActionProps {
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

export function ToolCardCopyAction({
  getText,
  tooltip,
  copiedTooltip,
  successMessage,
  failureMessage,
  ariaLabel,
  className,
  disabled,
  showSuccessNotification,
}: ToolCardCopyActionProps) {
  const { copied, copy } = useCopyTextAction({
    getText,
    successMessage,
    failureMessage,
    showSuccessNotification,
  });

  const label = copied ? (copiedTooltip ?? successMessage) : tooltip;

  return (
    <IconButton
      aria-label={ariaLabel ?? label}
      className={className}
      data-bf-state={copied ? 'copied' : undefined}
      disabled={disabled}
      icon={copied ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
      onClick={copy}
      size="sm"
      title={label}
      variant="quiet"
    />
  );
}
