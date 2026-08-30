// Compatibility entry point for product menus not yet moved to MenuPopover.
// Pointer corridor and timer behavior are owned by the portable UI package.
export {
  useSubmenuIntent,
  isPointInSubmenuBridge,
  isPointerMovingTowardSubmenu,
  type SubmenuIntentPoint,
  type SubmenuIntentRect,
  type UseSubmenuIntentOptions,
  type SubmenuIntentControls,
} from '@bitfun/ui';
