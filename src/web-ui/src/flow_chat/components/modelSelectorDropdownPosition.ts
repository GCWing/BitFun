export type ModelSelectorDropdownPlacement = 'top' | 'bottom';

export interface ModelSelectorDropdownAnchorRect {
  left: number;
  top: number;
  bottom: number;
  width: number;
}

export interface ModelSelectorDropdownViewport {
  width: number;
  height: number;
}

export interface ModelSelectorDropdownStyle {
  position: 'fixed';
  visibility: 'visible';
  left: string;
  top: string;
  bottom: string;
  width: string;
  minWidth: string;
  maxWidth: string;
}

const DROPDOWN_GAP_PX = 6;
const DROPDOWN_MAX_WIDTH_PX = 280;
const DROPDOWN_MIN_WIDTH_PX = 220;
const VIEWPORT_PADDING_PX = 8;

const clamp = (value: number, min: number, max: number): number => {
  return Math.min(Math.max(value, min), Math.max(min, max));
};

export function getModelSelectorDropdownStyle(
  rect: ModelSelectorDropdownAnchorRect,
  placement: ModelSelectorDropdownPlacement,
  viewport: ModelSelectorDropdownViewport,
): ModelSelectorDropdownStyle {
  const availableWidth = Math.max(viewport.width - VIEWPORT_PADDING_PX * 2, 1);
  const preferredWidth = Math.min(
    DROPDOWN_MAX_WIDTH_PX,
    Math.max(DROPDOWN_MIN_WIDTH_PX, rect.width),
  );
  const width = Math.min(preferredWidth, availableWidth);
  const maxLeft = viewport.width - VIEWPORT_PADDING_PX - width;
  const left = clamp(rect.left, VIEWPORT_PADDING_PX, maxLeft);

  const placementStyle = placement === 'bottom'
    ? { top: `${rect.bottom + DROPDOWN_GAP_PX}px`, bottom: 'auto' }
    : { top: 'auto', bottom: `${viewport.height - rect.top + DROPDOWN_GAP_PX}px` };

  return {
    position: 'fixed',
    visibility: 'visible',
    left: `${left}px`,
    width: `${width}px`,
    minWidth: '0',
    maxWidth: `${availableWidth}px`,
    ...placementStyle,
  };
}
