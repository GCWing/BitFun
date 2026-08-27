import {
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type HTMLAttributes,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  type RefObject,
} from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { IconButton, type IconButtonProps } from "../IconButton";
import { classNames } from "../../internal/classNames";
import { isImeOwnedKeyboardEvent } from "../../internal/ime";
import styles from "./Modal.module.css";

const MODAL_EXIT_DURATION_MS = 180;
const FOCUSABLE_SELECTOR = [
  "button:not([disabled])",
  "[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

export type ModalBackdropBlur = "none" | "subtle" | "base";
export type ModalBorder = "none" | "subtle" | "default";
export type ModalCloseReason = "close-button" | "escape-key" | "overlay";
export type ModalContentLayout = "scroll" | "flex" | "fill";
export type ModalContentPadding = "none" | "sm" | "md" | "lg" | "xl";
export type ModalElevation = "none" | "raised" | "overlay";
export type ModalPlacement = "center" | "bottom-left" | "bottom-right";
export type ModalRadius = "reference" | "base" | "lg" | "xl" | "2xl" | "3xl" | "4xl";
export type ModalSize = "small" | "medium" | "large" | "xlarge" | "xxlarge" | "wide";
export type ModalPortalContainer = Element | DocumentFragment;
export type ModalPortalTarget = ModalPortalContainer | (() => ModalPortalContainer | null) | null;

interface ModalContextValue {
  closeLabel: string;
  portalContainer?: ModalPortalTarget;
}

const ModalContext = createContext<ModalContextValue>({
  closeLabel: "Close dialog",
});

export interface ModalProviderProps {
  children: ReactNode;
  closeLabel?: string;
  portalContainer?: ModalPortalTarget;
}

export function ModalProvider({
  children,
  closeLabel = "Close dialog",
  portalContainer,
}: ModalProviderProps) {
  const value = useMemo<ModalContextValue>(
    () => ({ closeLabel, portalContainer }),
    [closeLabel, portalContainer],
  );

  return <ModalContext.Provider value={value}>{children}</ModalContext.Provider>;
}

export interface ModalProps {
  ariaDescribedBy?: string;
  ariaLabel?: string;
  ariaLabelledBy?: string;
  autoFocus?: boolean;
  backdropBlur?: ModalBackdropBlur;
  border?: ModalBorder;
  children: ReactNode;
  closeButtonProps?: Omit<IconButtonProps, "aria-label" | "icon" | "onClick">;
  closeButtonLabel?: string;
  closeButtonTestId?: string;
  closeIcon?: ReactNode;
  closeOnEscape?: boolean;
  closeOnOverlayClick?: boolean;
  contentClassName?: string;
  contentLayout?: ModalContentLayout;
  contentPadding?: ModalContentPadding;
  description?: ReactNode;
  dialogClassName?: string;
  draggable?: boolean;
  elevation?: ModalElevation;
  footer?: ReactNode;
  footerClassName?: string;
  headerActions?: ReactNode;
  initialFocusRef?: RefObject<HTMLElement | null>;
  isOpen: boolean;
  onClose: (reason: ModalCloseReason) => void;
  overlayClassName?: string;
  placement?: ModalPlacement;
  portalContainer?: ModalPortalTarget;
  portalled?: boolean;
  preventScroll?: boolean;
  radius?: ModalRadius;
  resizable?: boolean;
  role?: "dialog" | "alertdialog";
  showCloseButton?: boolean;
  showScrollbar?: boolean;
  size?: ModalSize;
  testId?: string;
  title?: ReactNode;
  titleExtra?: ReactNode;
  titleProps?: Omit<HTMLAttributes<HTMLHeadingElement>, "children" | "className" | "id">;
  titleTestId?: string;
  trapFocus?: boolean;
}

interface Point {
  x: number;
  y: number;
}

interface Dimensions {
  height: number;
  width: number;
}

interface DragStart extends Point {
  pointerId: number;
}

interface ResizeStart extends Dimensions, Point {
  pointerId: number;
}

const scrollLockCounts = new WeakMap<Document, number>();
const scrollLockPreviousOverflow = new WeakMap<Document, string>();
const modalStacks = new WeakMap<Document, symbol[]>();

function lockDocumentScroll(ownerDocument: Document): () => void {
  const nextCount = (scrollLockCounts.get(ownerDocument) ?? 0) + 1;
  if (nextCount === 1) {
    scrollLockPreviousOverflow.set(ownerDocument, ownerDocument.body.style.overflow);
  }
  scrollLockCounts.set(ownerDocument, nextCount);
  ownerDocument.body.style.overflow = "hidden";

  let released = false;
  return () => {
    if (released) return;
    released = true;
    const count = Math.max(0, (scrollLockCounts.get(ownerDocument) ?? 1) - 1);
    if (count === 0) {
      scrollLockCounts.delete(ownerDocument);
      ownerDocument.body.style.overflow = scrollLockPreviousOverflow.get(ownerDocument) ?? "";
      scrollLockPreviousOverflow.delete(ownerDocument);
    } else {
      scrollLockCounts.set(ownerDocument, count);
    }
  };
}

function resolvePortalContainer(target: ModalPortalTarget | undefined): ModalPortalContainer | null {
  if (typeof target === "function") return target();
  if (target) return target;
  return typeof document === "undefined" ? null : document.body;
}

function registerModal(ownerDocument: Document, identity: symbol): () => void {
  const stack = modalStacks.get(ownerDocument) ?? [];
  stack.push(identity);
  modalStacks.set(ownerDocument, stack);

  return () => {
    const current = modalStacks.get(ownerDocument);
    if (!current) return;
    const index = current.lastIndexOf(identity);
    if (index >= 0) current.splice(index, 1);
    if (current.length === 0) modalStacks.delete(ownerDocument);
  };
}

function isTopModal(ownerDocument: Document, identity: symbol): boolean {
  const stack = modalStacks.get(ownerDocument);
  return stack?.[stack.length - 1] === identity;
}

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
    .filter((element) => element.getAttribute("aria-hidden") !== "true");
}

export const Modal = forwardRef<HTMLDivElement, ModalProps>(function Modal({
  ariaDescribedBy,
  ariaLabel,
  ariaLabelledBy,
  autoFocus = true,
  backdropBlur = "base",
  border = "subtle",
  children,
  closeButtonProps,
  closeButtonLabel,
  closeButtonTestId,
  closeIcon,
  closeOnEscape = true,
  closeOnOverlayClick = true,
  contentClassName,
  contentLayout = "scroll",
  contentPadding = "none",
  description,
  dialogClassName,
  draggable = false,
  elevation = "overlay",
  footer,
  footerClassName,
  headerActions,
  initialFocusRef,
  isOpen,
  onClose,
  overlayClassName,
  placement = "center",
  portalContainer,
  portalled = true,
  preventScroll = true,
  radius = "reference",
  resizable = false,
  role = "dialog",
  showCloseButton = true,
  showScrollbar = true,
  size = "medium",
  testId,
  title,
  titleExtra,
  titleProps,
  titleTestId,
  trapFocus = true,
}, forwardedRef) {
  const context = useContext(ModalContext);
  const resolvedPortalContainer = resolvePortalContainer(
    portalContainer === undefined ? context.portalContainer : portalContainer,
  );
  const ownerDocument = resolvedPortalContainer?.ownerDocument
    ?? (typeof document === "undefined" ? null : document);
  const identityRef = useRef(Symbol("bitfun-modal"));
  const modalRef = useRef<HTMLDivElement | null>(null);
  const headerRef = useRef<HTMLDivElement | null>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const pointerStartedOnOverlayRef = useRef(false);
  const dragStartRef = useRef<DragStart | null>(null);
  const resizeStartRef = useRef<ResizeStart | null>(null);
  const resizeDirectionRef = useRef<string>("");
  const generatedTitleId = useId();
  const generatedDescriptionId = useId();
  const [isPresent, setIsPresent] = useState(isOpen);
  const [isDragging, setIsDragging] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const [position, setPosition] = useState<Point | null>(null);
  const [dimensions, setDimensions] = useState<Dimensions | null>(null);
  const isExiting = !isOpen && isPresent;
  const resolvedCloseLabel = closeButtonLabel ?? context.closeLabel;

  const setModalRef = useCallback((node: HTMLDivElement | null) => {
    modalRef.current = node;
    if (typeof forwardedRef === "function") {
      forwardedRef(node);
    } else if (forwardedRef) {
      forwardedRef.current = node;
    }
  }, [forwardedRef]);

  useEffect(() => {
    if (isOpen) {
      setIsPresent(true);
      return;
    }
    if (!isPresent) return;
    const view = ownerDocument?.defaultView;
    if (!view) {
      setIsPresent(false);
      return;
    }
    const timer = view.setTimeout(() => setIsPresent(false), MODAL_EXIT_DURATION_MS);
    return () => view.clearTimeout(timer);
  }, [isOpen, isPresent, ownerDocument]);

  useEffect(() => {
    if ((!isOpen && !isPresent) || !ownerDocument || !preventScroll) return;
    return lockDocumentScroll(ownerDocument);
  }, [isOpen, isPresent, ownerDocument, preventScroll]);

  useEffect(() => {
    if (!isOpen || !ownerDocument) return;
    return registerModal(ownerDocument, identityRef.current);
  }, [isOpen, ownerDocument]);

  useEffect(() => {
    if (!isOpen || !ownerDocument) return;
    const handleEscape = (event: KeyboardEvent) => {
      if (
        event.key !== "Escape"
        || isImeOwnedKeyboardEvent(event)
        || !closeOnEscape
        || !isTopModal(ownerDocument, identityRef.current)
      ) {
        return;
      }
      event.preventDefault();
      onClose("escape-key");
    };
    ownerDocument.addEventListener("keydown", handleEscape);
    return () => ownerDocument.removeEventListener("keydown", handleEscape);
  }, [closeOnEscape, isOpen, onClose, ownerDocument]);

  useEffect(() => {
    if (!isOpen || !ownerDocument) return;
    const HTMLElementConstructor = ownerDocument.defaultView?.HTMLElement;
    previousFocusRef.current = HTMLElementConstructor
      && ownerDocument.activeElement instanceof HTMLElementConstructor
      ? ownerDocument.activeElement
      : null;
    const dialog = modalRef.current;
    if (autoFocus) {
      const focusTarget = initialFocusRef?.current
        ?? (dialog ? getFocusableElements(dialog)[0] : null)
        ?? dialog;
      focusTarget?.focus();
    }

    const handleFocusTrap = (event: KeyboardEvent) => {
      if (
        event.key !== "Tab"
        || !dialog
        || !isTopModal(ownerDocument, identityRef.current)
      ) {
        return;
      }
      const elements = getFocusableElements(dialog);
      if (elements.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      const first = elements[0];
      const last = elements[elements.length - 1];
      if (!first || !last) return;
      if (event.shiftKey && ownerDocument.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && ownerDocument.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    if (trapFocus) ownerDocument.addEventListener("keydown", handleFocusTrap);
    return () => {
      if (trapFocus) ownerDocument.removeEventListener("keydown", handleFocusTrap);
      if (previousFocusRef.current?.isConnected) previousFocusRef.current.focus();
      previousFocusRef.current = null;
    };
  }, [autoFocus, initialFocusRef, isOpen, ownerDocument, trapFocus]);

  const requestClose = useCallback((reason: ModalCloseReason) => {
    if (!isOpen) return;
    onClose(reason);
  }, [isOpen, onClose]);

  const handleOverlayPointerDown = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    pointerStartedOnOverlayRef.current = event.currentTarget === event.target;
  }, []);

  const handleOverlayClick = useCallback((event: ReactMouseEvent<HTMLDivElement>) => {
    const shouldClose = closeOnOverlayClick
      && pointerStartedOnOverlayRef.current
      && event.currentTarget === event.target
      && ownerDocument
      && isTopModal(ownerDocument, identityRef.current);
    pointerStartedOnOverlayRef.current = false;
    if (shouldClose) requestClose("overlay");
  }, [closeOnOverlayClick, ownerDocument, requestClose]);

  useEffect(() => {
    const view = ownerDocument?.defaultView;
    if (!isOpen || (!draggable && !resizable) || !view) return;
    const frame = view.requestAnimationFrame(() => {
      const dialog = modalRef.current;
      if (!dialog) return;
      const rect = dialog.getBoundingClientRect();
      setPosition({
        x: Math.max(0, (view.innerWidth - rect.width) / 2),
        y: Math.max(0, (view.innerHeight - rect.height) / 2),
      });
      if (resizable) setDimensions({ height: rect.height, width: rect.width });
    });
    return () => view.cancelAnimationFrame(frame);
  }, [draggable, isOpen, ownerDocument, resizable]);

  useEffect(() => {
    if (isOpen || isPresent) return;
    setDimensions(null);
    setPosition(null);
    setIsDragging(false);
    setIsResizing(false);
    dragStartRef.current = null;
    resizeStartRef.current = null;
  }, [isOpen, isPresent]);

  const endPointerInteraction = useCallback(() => {
    setIsDragging(false);
    setIsResizing(false);
    dragStartRef.current = null;
    resizeStartRef.current = null;
    resizeDirectionRef.current = "";
  }, []);

  useEffect(() => {
    if ((!isDragging && !isResizing) || !ownerDocument) return;
    const view = ownerDocument.defaultView;
    if (!view) return;

    const handlePointerMove = (event: PointerEvent) => {
      const dialog = modalRef.current;
      if (!dialog) return;

      const dragStart = dragStartRef.current;
      if (isDragging && dragStart?.pointerId === event.pointerId) {
        const width = dialog.offsetWidth;
        const height = dialog.offsetHeight;
        setPosition({
          x: Math.max(0, Math.min(event.clientX - dragStart.x, view.innerWidth - width)),
          y: Math.max(0, Math.min(event.clientY - dragStart.y, view.innerHeight - height)),
        });
        return;
      }

      const resizeStart = resizeStartRef.current;
      if (!isResizing || !resizeStart || resizeStart.pointerId !== event.pointerId || !position) {
        return;
      }
      const deltaX = event.clientX - resizeStart.x;
      const deltaY = event.clientY - resizeStart.y;
      const direction = resizeDirectionRef.current;
      let width = resizeStart.width;
      let height = resizeStart.height;
      let x = position.x;
      let y = position.y;

      if (direction.includes("e")) width = Math.max(300, resizeStart.width + deltaX);
      if (direction.includes("w")) {
        width = Math.max(300, resizeStart.width - deltaX);
        x = position.x + (resizeStart.width - width);
      }
      if (direction.includes("s")) height = Math.max(200, resizeStart.height + deltaY);
      if (direction.includes("n")) {
        height = Math.max(200, resizeStart.height - deltaY);
        y = position.y + (resizeStart.height - height);
      }
      if (x < 0) {
        width += x;
        x = 0;
      }
      if (y < 0) {
        height += y;
        y = 0;
      }
      width = Math.min(width, view.innerWidth - x);
      height = Math.min(height, view.innerHeight - y);
      setDimensions({ height, width });
      setPosition({ x, y });
    };

    view.addEventListener("pointermove", handlePointerMove);
    view.addEventListener("pointerup", endPointerInteraction);
    view.addEventListener("pointercancel", endPointerInteraction);
    ownerDocument.body.style.userSelect = "none";
    return () => {
      view.removeEventListener("pointermove", handlePointerMove);
      view.removeEventListener("pointerup", endPointerInteraction);
      view.removeEventListener("pointercancel", endPointerInteraction);
      ownerDocument.body.style.userSelect = "";
    };
  }, [endPointerInteraction, isDragging, isResizing, ownerDocument, position]);

  const handleDragStart = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    if (!draggable || !modalRef.current) return;
    if ((event.target as Element).closest('[data-bf-part="close"]')) return;
    const rect = modalRef.current.getBoundingClientRect();
    dragStartRef.current = {
      pointerId: event.pointerId,
      x: event.clientX - rect.left,
      y: event.clientY - rect.top,
    };
    setIsDragging(true);
    event.preventDefault();
  }, [draggable]);

  const handleResizeStart = useCallback((
    event: ReactPointerEvent<HTMLDivElement>,
    direction: string,
  ) => {
    if (!resizable || !modalRef.current) return;
    const rect = modalRef.current.getBoundingClientRect();
    resizeStartRef.current = {
      height: rect.height,
      pointerId: event.pointerId,
      width: rect.width,
      x: event.clientX,
      y: event.clientY,
    };
    resizeDirectionRef.current = direction;
    setIsResizing(true);
    event.preventDefault();
    event.stopPropagation();
  }, [resizable]);

  if (!isPresent || (portalled && !resolvedPortalContainer)) return null;

  const positionedStyle: CSSProperties | undefined = (draggable || resizable) && position
    ? {
        height: dimensions?.height,
        left: position.x,
        margin: 0,
        maxHeight: "none",
        position: "fixed",
        top: position.y,
        transform: "none",
        width: dimensions?.width,
      }
    : undefined;
  const dialogState = [
    draggable && "draggable",
    isDragging && "dragging",
    resizable && "resizable",
    isResizing && "resizing",
    isExiting && "exiting",
  ].filter(Boolean).join(" ");
  const hasHeader = Boolean(title || description || headerActions || showCloseButton || draggable);
  const resolvedDescribedBy = [
    ariaDescribedBy,
    description !== undefined && description !== null ? generatedDescriptionId : undefined,
  ].filter(Boolean).join(" ") || undefined;

  const modal = (
    <div
      className={classNames(styles.overlay, overlayClassName)}
      data-bf-backdrop-blur={backdropBlur}
      data-bf-component="modal"
      data-bf-part="overlay"
      data-bf-placement={placement}
      data-bf-state={isExiting ? "exiting" : undefined}
      onAnimationEnd={(event) => {
        if (isExiting && event.currentTarget === event.target) setIsPresent(false);
      }}
      onClick={handleOverlayClick}
      onPointerDown={handleOverlayPointerDown}
    >
      <div
        aria-describedby={resolvedDescribedBy}
        aria-hidden={isExiting || undefined}
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy ?? (title ? generatedTitleId : undefined)}
        aria-modal="true"
        className={classNames(styles.dialog, dialogClassName)}
        data-bf-border={border}
        data-bf-component="modal"
        data-bf-elevation={elevation}
        data-bf-has-footer={footer !== undefined && footer !== null ? "true" : "false"}
        data-bf-part="dialog"
        data-bf-placement={placement}
        data-bf-radius={radius}
        data-bf-size={size}
        data-bf-state={dialogState || undefined}
        data-positioned={positionedStyle ? "true" : "false"}
        data-testid={testId}
        ref={setModalRef}
        role={role}
        style={positionedStyle}
        tabIndex={-1}
      >
        {hasHeader && (
          <div
            className={styles.headerShell}
            data-bf-component="modal"
            data-bf-has-title={title || description ? "true" : "false"}
            data-bf-part="headerShell"
          >
            {(title || description || draggable) && (
              <div
                className={styles.header}
                data-bf-component="modal"
                data-bf-part="header"
                data-bf-state={draggable ? "draggable" : undefined}
                onPointerDown={handleDragStart}
                ref={headerRef}
              >
                {(title || description) && (
                  <div className={styles.heading} data-bf-component="modal" data-bf-part="heading">
                    {title && (
                      <div className={styles.titleGroup} data-bf-component="modal" data-bf-part="titleGroup">
                        <h2
                          {...titleProps}
                          className={styles.title}
                          data-bf-component="modal"
                          data-bf-part="title"
                          data-testid={titleTestId}
                          id={generatedTitleId}
                        >
                          {title}
                        </h2>
                        {titleExtra && (
                          <span className={styles.titleExtra} data-bf-component="modal" data-bf-part="titleExtra">
                            {titleExtra}
                          </span>
                        )}
                      </div>
                    )}
                    {description !== undefined && description !== null && (
                      <p
                        className={styles.description}
                        data-bf-component="modal"
                        data-bf-part="description"
                        id={generatedDescriptionId}
                      >
                        {description}
                      </p>
                    )}
                  </div>
                )}
              </div>
            )}
            {headerActions !== undefined && headerActions !== null && (
              <div className={styles.headerActions} data-bf-component="modal" data-bf-part="headerActions">
                {headerActions}
              </div>
            )}
            {showCloseButton && (
              <IconButton
                {...closeButtonProps}
                aria-label={resolvedCloseLabel}
                className={classNames(styles.close, closeButtonProps?.className)}
                data-bf-part="close"
                data-testid={closeButtonTestId}
                icon={closeIcon ?? <X aria-hidden="true" />}
                onClick={() => requestClose("close-button")}
                size="sm"
                variant="quiet"
              />
            )}
          </div>
        )}

        <div
          className={classNames(styles.content, contentClassName)}
          data-bf-component="modal"
          data-bf-layout={contentLayout}
          data-bf-part="content"
          data-bf-padding={contentPadding}
          data-bf-show-scrollbar={showScrollbar ? "true" : "false"}
        >
          {children}
        </div>

        {footer !== undefined && footer !== null && (
          <footer
            className={classNames(styles.footer, footerClassName)}
            data-bf-component="modal"
            data-bf-part="footer"
          >
            {footer}
          </footer>
        )}

        {resizable && (["n", "s", "w", "e", "nw", "ne", "sw", "se"] as const).map((direction) => (
          <div
            className={classNames(styles.resizeHandle, styles[`resizeHandle${direction.toUpperCase()}`])}
            data-bf-component="modal"
            data-bf-part="resizeHandle"
            data-bf-resize-direction={direction}
            data-bf-state={isResizing ? "resizing" : "resizable"}
            key={direction}
            onPointerDown={(event) => handleResizeStart(event, direction)}
          />
        ))}
      </div>
    </div>
  );

  return portalled && resolvedPortalContainer
    ? createPortal(modal, resolvedPortalContainer)
    : modal;
});
