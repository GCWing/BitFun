export type OverlayPortalContainer = Element | DocumentFragment;

export type OverlayPortalTarget =
  | OverlayPortalContainer
  | (() => OverlayPortalContainer | null)
  | null;

export type OverlayDismissReason =
  | "escape-key"
  | "pointer-outside"
  | "programmatic";

export type OverlayLayerScope = string;
