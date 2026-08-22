import {
  computeFixedPopoverPositionInViewport,
  DEFAULT_POPOVER_VIEWPORT_PADDING,
  type FixedPopoverAlignment,
  type FixedPopoverPlacement,
  type FixedPopoverViewport,
} from '@/shared/utils/fixedPopoverViewport';

const MODEL_SELECTOR_DROPDOWN_GAP = 6;

interface ModelSelectorDropdownAnchorRect {
  left: number;
  right?: number;
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
  maxHeight: string;
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
  alignment: FixedPopoverAlignment = 'start',
): ModelSelectorDropdownLayout {
  const availableHeight = (placement: FixedPopoverPlacement): number => {
    const height = placement === 'top'
      ? anchorRect.top - MODEL_SELECTOR_DROPDOWN_GAP - DEFAULT_POPOVER_VIEWPORT_PADDING
      : viewport.height
        - anchorRect.bottom
        - MODEL_SELECTOR_DROPDOWN_GAP
        - DEFAULT_POPOVER_VIEWPORT_PADDING;
    return Math.max(0, height);
  };
  const alternatePlacement = preferredPlacement === 'top' ? 'bottom' : 'top';
  const preferredAvailableHeight = availableHeight(preferredPlacement);
  const alternateAvailableHeight = availableHeight(alternatePlacement);
  const placement = dropdownSize.height <= preferredAvailableHeight
    ? preferredPlacement
    : dropdownSize.height <= alternateAvailableHeight
      ? alternatePlacement
      : preferredAvailableHeight >= alternateAvailableHeight
        ? preferredPlacement
        : alternatePlacement;
  const maxHeight = availableHeight(placement);
  const renderedHeight = Math.min(dropdownSize.height, maxHeight);
  const position = computeFixedPopoverPositionInViewport(
    anchorRect,
    dropdownSize.width,
    renderedHeight,
    viewport,
    {
      preferredPlacement: placement,
      alignment,
      gap: MODEL_SELECTOR_DROPDOWN_GAP,
      padding: DEFAULT_POPOVER_VIEWPORT_PADDING,
    },
  );

  return {
    placement,
    style: {
      position: 'fixed',
      visibility: 'visible',
      left: `${position.left}px`,
      top: `${position.top}px`,
      bottom: 'auto',
      maxHeight: `${maxHeight}px`,
    },
  };
}

export function getModelSelectorDropdownStyle(
  anchorRect: ModelSelectorDropdownAnchorRect,
  dropdownSize: ModelSelectorDropdownSize,
  preferredPlacement: FixedPopoverPlacement,
  viewport: FixedPopoverViewport,
  alignment: FixedPopoverAlignment = 'start',
): ModelSelectorDropdownStyle {
  return getModelSelectorDropdownLayout(
    anchorRect,
    dropdownSize,
    preferredPlacement,
    viewport,
    alignment,
  ).style;
}
