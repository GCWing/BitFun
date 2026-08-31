import { type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useDesignSystem } from "./useDesignSystem";
import type {
  OverlayPortalContainer,
  OverlayPortalTarget,
} from "./types";

export function resolvePortalTarget(
  target: OverlayPortalTarget | undefined,
  fallbackDocument?: Document | null,
): OverlayPortalContainer | null {
  if (typeof target === "function") return target();
  if (target) return target;
  const ownerDocument = fallbackDocument
    ?? (typeof document === "undefined" ? null : document);
  return ownerDocument?.body ?? null;
}

export interface PortalProps {
  children: ReactNode;
  ownerDocument?: Document | null;
  target?: OverlayPortalTarget;
}

export function Portal({ children, ownerDocument, target }: PortalProps) {
  const designSystem = useDesignSystem();
  const container = resolvePortalTarget(
    target === undefined ? designSystem.portalHost : target,
    ownerDocument,
  );
  return container ? createPortal(children, container) : null;
}
