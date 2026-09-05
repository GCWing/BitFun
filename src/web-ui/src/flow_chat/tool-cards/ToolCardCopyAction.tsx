
import { ToolCardCopyButton } from '@openbitfun/ui/flow-chat';
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

  return (
    <ToolCardCopyButton
      className={className}
      copied={copied}
      copiedLabel={copiedTooltip ?? successMessage}
      disabled={disabled}
      label={ariaLabel ?? tooltip}
      onPress={copy}
    />
  );
}
