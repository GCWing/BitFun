import {
  computeFixedPopoverPositionInViewport,
  type FixedPopoverPlacement,
  type FixedPopoverViewport,
} from '@/shared/utils/fixedPopoverViewport';

interface ModelSelectorDropdownAnchorRect {
  left: number;
  top: number;
  bottom: number;
}

interface ModelSelectorDropdownSize {
  width: number;
  height: number;
}

interface ModelSelectorDropdownStyle {
  position: 'fixed';
  visibility: 'visible';
  left: string;
  top: string;
  bottom: 'auto';
}

export interface ModelSelectorDropdownLayout {
  style: ModelSelectorDropdownStyle;
  placement: FixedPopoverPlacement;
}

export function getModelSelectorDropdownLayout(
  anchorRect: ModelSelectorDropdownAnchorRect,
  dropdownSize: ModelSelectorDropdownSize,
  preferredPlacement: FixedPopoverPlacement,
  viewport: FixedPopoverViewport,
): ModelSelectorDropdownLayout {
  const position = computeFixedPopoverPositionInViewport(
    anchorRect,
    dropdownSize.width,
    dropdownSize.height,
    viewport,
    { preferredPlacement },
  );
  const placement = position.top + dropdownSize.height <= anchorRect.top
    ? 'top'
    : position.top >= anchorRect.bottom
      ? 'bottom'
      : preferredPlacement;

  return {
    placement,
    style: {
      position: 'fixed',
      visibility: 'visible',
      left: `${position.left}px`,
      top: `${position.top}px`,
      bottom: 'auto',
    },
  };
}

export function getModelSelectorDropdownStyle(
  anchorRect: ModelSelectorDropdownAnchorRect,
  dropdownSize: ModelSelectorDropdownSize,
  preferredPlacement: FixedPopoverPlacement,
  viewport: FixedPopoverViewport,
): ModelSelectorDropdownStyle {
  return getModelSelectorDropdownLayout(
    anchorRect,
    dropdownSize,
    preferredPlacement,
    viewport,
  ).style;
}
