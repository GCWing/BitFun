import React from 'react';
import { X } from 'lucide-react';
import type { IconButtonProps } from '../IconButton';
import { Tooltip } from '../Tooltip';
import '../IconButton/IconButton.scss';
import './PopupCloseButton.scss';

export interface PopupCloseButtonProps extends Omit<
  IconButtonProps,
  'children' | 'isLoading' | 'shape' | 'size' | 'type' | 'variant'
> {
  'aria-label': string;
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
        'icon-btn',
        'icon-btn--medium',
        'icon-btn--ghost',
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
