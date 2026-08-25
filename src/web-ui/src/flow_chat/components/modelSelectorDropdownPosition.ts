import {
  computeFixedPopoverLeftInViewport,
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
  /** `auto` for a top-placed menu, which is held by its bottom edge instead. */
  top: string;
  /** `auto` for a bottom-placed menu, which is held by its top edge instead. */
  bottom: string;
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
  const left = computeFixedPopoverLeftInViewport(
    anchorRect,
    dropdownSize.width,
    viewport.width,
    { alignment, padding: DEFAULT_POPOVER_VIEWPORT_PADDING },
  );

  // The edge next to the trigger is pinned to the trigger, and `maxHeight`
  // already keeps the free edge inside the viewport. Deriving that pinned edge
  // from the measured height instead would move the menu against the trigger
  // for one frame every time its content changes height, and only snap back
  // once a resize observation has been turned into a new position.
  const verticalEdge = placement === 'top'
    ? {
      top: 'auto',
      bottom: `${viewport.height - anchorRect.top + MODEL_SELECTOR_DROPDOWN_GAP}px`,
    }
    : {
      top: `${anchorRect.bottom + MODEL_SELECTOR_DROPDOWN_GAP}px`,
      bottom: 'auto',
    };

  return {
    placement,
    style: {
      position: 'fixed',
      visibility: 'visible',
      left: `${left}px`,
      ...verticalEdge,
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
