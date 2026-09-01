import {
  useEffect,
  useRef,
  type RefObject,
} from "react";
import { isImeOwnedKeyboardEvent } from "../internal/ime";
import { useOverlayLayerStack } from "./LayerStack";
import type {
  OverlayDismissReason,
  OverlayLayerScope,
} from "./types";

export interface UseDismissibleLayerOptions {
  branchRefs?: readonly RefObject<HTMLElement | null>[];
  containsTarget?: (target: Node) => boolean;
  dismissOnEscape?: boolean;
  dismissOnPointerOutside?: boolean;
  enabled: boolean;
  layerRef: RefObject<HTMLElement | null>;
  onDismiss: (reason: OverlayDismissReason) => void;
  ownerDocument?: Document | null;
  scope?: OverlayLayerScope;
}

function isNode(value: EventTarget | null): value is Node {
  return Boolean(value && typeof (value as Node).nodeType === "number");
}

export function useDismissibleLayer({
  branchRefs = [],
  containsTarget,
  dismissOnEscape = true,
  dismissOnPointerOutside = true,
  enabled,
  layerRef,
  onDismiss,
  ownerDocument,
  scope,
}: UseDismissibleLayerOptions): symbol {
  const stack = useOverlayLayerStack();
  const identityRef = useRef(Symbol("bitfun-overlay-layer"));
  const onDismissRef = useRef(onDismiss);
  onDismissRef.current = onDismiss;

  useEffect(() => {
    if (!enabled) return;
    return stack.register({
      id: identityRef.current,
      onDismiss: (reason) => onDismissRef.current(reason),
      scope,
    });
  }, [enabled, scope, stack]);

  useEffect(() => {
    if (!enabled) return;
    const documentOwner = ownerDocument
      ?? layerRef.current?.ownerDocument
      ?? (typeof document === "undefined" ? null : document);
    if (!documentOwner) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        event.key !== "Escape"
        || isImeOwnedKeyboardEvent(event)
        || !dismissOnEscape
        || !stack.isTopLayer(identityRef.current)
      ) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      onDismissRef.current("escape-key");
    };

    const handlePointerDown = (event: PointerEvent) => {
      if (
        !dismissOnPointerOutside
        || !stack.isTopLayer(identityRef.current)
        || !isNode(event.target)
      ) {
        return;
      }
      const target = event.target;
      if (!isNode(target)) return;
      if (layerRef.current?.contains(target)) return;
      if (branchRefs.some((branchRef) => branchRef.current?.contains(target))) return;
      if (containsTarget?.(target)) return;
      onDismissRef.current("pointer-outside");
    };

    documentOwner.addEventListener("keydown", handleKeyDown, true);
    documentOwner.addEventListener("pointerdown", handlePointerDown, true);
    return () => {
      documentOwner.removeEventListener("keydown", handleKeyDown, true);
      documentOwner.removeEventListener("pointerdown", handlePointerDown, true);
    };
  }, [branchRefs, containsTarget, dismissOnEscape, dismissOnPointerOutside, enabled, layerRef, ownerDocument, stack]);

  return identityRef.current;
}
