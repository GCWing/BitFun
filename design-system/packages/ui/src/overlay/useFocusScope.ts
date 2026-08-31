import {
  useEffect,
  useLayoutEffect,
  type RefObject,
} from "react";

const useIsomorphicLayoutEffect = typeof window === "undefined"
  ? useEffect
  : useLayoutEffect;

const FOCUSABLE_SELECTOR = [
  "button:not([disabled])",
  "[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

function focusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
    .filter((element) => element.getAttribute("aria-hidden") !== "true");
}

export interface UseFocusScopeOptions {
  active: boolean;
  autoFocus?: boolean;
  containerRef: RefObject<HTMLElement | null>;
  initialFocusRef?: RefObject<HTMLElement | null>;
  ownerDocument?: Document | null;
  restoreFocus?: boolean;
  trapFocus?: boolean;
}

export function useFocusScope({
  active,
  autoFocus = true,
  containerRef,
  initialFocusRef,
  ownerDocument,
  restoreFocus = true,
  trapFocus = true,
}: UseFocusScopeOptions): void {
  useIsomorphicLayoutEffect(() => {
    if (!active) return;
    const container = containerRef.current;
    const documentOwner = ownerDocument
      ?? container?.ownerDocument
      ?? (typeof document === "undefined" ? null : document);
    if (!container || !documentOwner) return;
    const HTMLElementConstructor = documentOwner.defaultView?.HTMLElement;
    const previousFocus = HTMLElementConstructor
      && documentOwner.activeElement instanceof HTMLElementConstructor
      ? documentOwner.activeElement as HTMLElement
      : null;

    if (autoFocus) {
      const target = initialFocusRef?.current
        ?? focusableElements(container)[0]
        ?? container;
      target.focus();
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!trapFocus || event.key !== "Tab") return;
      const elements = focusableElements(container);
      if (elements.length === 0) {
        event.preventDefault();
        container.focus();
        return;
      }
      const first = elements[0];
      const last = elements[elements.length - 1];
      if (event.shiftKey && documentOwner.activeElement === first) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && documentOwner.activeElement === last) {
        event.preventDefault();
        first?.focus();
      }
    };

    documentOwner.addEventListener("keydown", handleKeyDown, true);
    return () => {
      documentOwner.removeEventListener("keydown", handleKeyDown, true);
      if (restoreFocus && previousFocus?.isConnected) previousFocus.focus();
    };
  }, [active, autoFocus, containerRef, initialFocusRef, ownerDocument, restoreFocus, trapFocus]);
}
