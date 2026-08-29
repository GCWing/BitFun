import React from 'react';
import { X } from 'lucide-react';
import { Tooltip } from '../Tooltip';
import './PopupCloseButton.scss';

export interface PopupCloseButtonProps extends Omit<
  React.ButtonHTMLAttributes<HTMLButtonElement>,
  'children' | 'size' | 'type'
> {
  'aria-label': string;
  tooltip?: React.ReactNode;
  tooltipPlacement?: 'top' | 'bottom' | 'left' | 'right';
  tooltipFollowCursor?: boolean;
}

/**
 * Canonical dismiss control for dialog and popup surfaces.
 *
 * Its 32px target and 16px icon are intentionally fixed. Placement belongs
 * to the popup chrome, which must keep the inline-end inset equal to the
 * block-start inset so product surfaces cannot introduce lopsided corners.
 */
export const PopupCloseButton = React.forwardRef<
  HTMLButtonElement,
  PopupCloseButtonProps
>(({
  'aria-label': ariaLabel,
  className = '',
  disabled,
  tooltip,
  tooltipFollowCursor = true,
  tooltipPlacement = 'top',
  ...props
}, ref) => {
  const button = (
    <button
      ref={ref}
      type="button"
      disabled={disabled}
      className={[
        'popup-close-button',
        className,
      ].filter(Boolean).join(' ')}
      data-bf-component="popup-close-button"
      data-bf-part="root"
      data-bf-variant="ghost"
      data-bf-size="medium"
      data-bf-shape="square"
      data-bf-role="popup-close"
      aria-label={ariaLabel}
      {...props}
    >
      <X aria-hidden="true" size={16} strokeWidth={2} />
    </button>
  );

  if (tooltip && !disabled) {
    return (
      <Tooltip
        content={tooltip}
        placement={tooltipPlacement}
        followCursor={tooltipFollowCursor}
      >
        {button}
      </Tooltip>
    );
  }

  return button;
});

PopupCloseButton.displayName = 'PopupCloseButton';
