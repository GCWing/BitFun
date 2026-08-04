import {
  computeFixedPopoverPositionInViewport,
  type FixedPopoverViewport,
} from '@/shared/utils/fixedPopoverViewport';

export interface MarketAccountMenuPosition {
  top: number;
  left: number;
}

export function calculateMarketAccountMenuPosition(
  triggerRect: Pick<DOMRect, 'top' | 'right' | 'bottom'>,
  menuRect: Pick<DOMRect, 'width' | 'height'>,
  viewport: FixedPopoverViewport,
): MarketAccountMenuPosition {
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
