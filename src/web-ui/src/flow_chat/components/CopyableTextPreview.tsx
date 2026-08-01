import React from 'react';
import { Check, Copy } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { IconButton, Tooltip } from '../../component-library';
import { useCopyTextAction } from '../hooks/useCopyTextAction';
import './CopyableTextPreview.scss';

interface CopyableTextPreviewProps extends React.HTMLAttributes<HTMLElement> {
  text?: string | null;
  emptyText: React.ReactNode;
  as?: 'span' | 'code';
  className?: string;
  tooltipContent?: React.ReactNode;
  tooltipPlacement?: 'top' | 'bottom' | 'left' | 'right';
}

export const CopyableTextPreview = React.forwardRef<HTMLElement, CopyableTextPreviewProps>(({
  text,
  emptyText,
  as = 'span',
  className,
  tooltipContent,
  tooltipPlacement = 'bottom',
  ...restProps
}, ref) => {
  const { t } = useTranslation('flow-chat');
  const content = text?.trim()
    ? text
    : <span className="copyable-text-preview__empty">{emptyText}</span>;
  const resolvedClassName = `copyable-text-preview${className ? ` ${className}` : ''}`;
  const copyText = typeof tooltipContent === 'string' && tooltipContent.trim()
    ? tooltipContent
    : undefined;
  const { copied, copy } = useCopyTextAction({
    getText: () => copyText ?? '',
    successMessage: t('toolCards.common.copied'),
    failureMessage: t('toolCards.common.copyFailed'),
    showSuccessNotification: false,
  });
  const copyTooltip = copied ? t('toolCards.common.copied') : t('toolCards.common.copy');
  const node = as === 'code' ? (
    <code ref={ref} className={resolvedClassName} {...restProps}>
      {content}
    </code>
  ) : (
    <span ref={ref} className={resolvedClassName} {...restProps}>
      {content}
    </span>
  );

  if (!tooltipContent) {
    return node;
  }

  return (
    <Tooltip
      content={
        <div className="copyable-text-preview-tooltip-content">
          <span className="copyable-text-preview-tooltip-content__text">{tooltipContent}</span>
          {copyText && (
            <IconButton
              className={`copyable-text-preview-tooltip__copy${copied ? ' copied' : ''}`}
              variant="ghost"
              size="xs"
              onClick={copy}
              tooltip={copyTooltip}
              aria-label={copyTooltip}
            >
              {copied ? <Check size={12} /> : <Copy size={12} />}
            </IconButton>
          )}
        </div>
      }
      placement={tooltipPlacement}
      className="copyable-text-preview-tooltip"
      interactive
    >
      {node}
    </Tooltip>
  );
});

CopyableTextPreview.displayName = 'CopyableTextPreview';
