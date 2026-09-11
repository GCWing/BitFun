import {
  computeFixedPopoverPositionInViewport,
  type FixedPopoverViewport,
} from '@/shared/utils/fixedPopoverViewport';

export interface AccountIdentityMenuPosition {
  top: number;
  left: number;
}

export function calculateAccountIdentityMenuPosition(
  triggerRect: Pick<DOMRect, 'top' | 'right' | 'bottom'>,
  menuRect: Pick<DOMRect, 'width' | 'height'>,
  viewport: FixedPopoverViewport,
): AccountIdentityMenuPosition {
  return computeFixedPopoverPositionInViewport(
    {
      left: triggerRect.right - menuRect.width,
      top: triggerRect.top,
      bottom: triggerRect.bottom,
    },
    menuRect.width,
    menuRect.height,
    viewport,
  );
}
