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

export function getModelSelectorDropdownStyle(
  anchorRect: ModelSelectorDropdownAnchorRect,
  dropdownSize: ModelSelectorDropdownSize,
  preferredPlacement: FixedPopoverPlacement,
  viewport: FixedPopoverViewport,
): ModelSelectorDropdownStyle {
  const position = computeFixedPopoverPositionInViewport(
    anchorRect,
    dropdownSize.width,
    dropdownSize.height,
    viewport,
    { preferredPlacement },
  );

  return {
    position: 'fixed',
    visibility: 'visible',
    left: `${position.left}px`,
    top: `${position.top}px`,
    bottom: 'auto',
  };
}
