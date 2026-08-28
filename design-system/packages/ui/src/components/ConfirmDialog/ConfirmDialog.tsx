import {
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  AlertCircle,
  CheckCircle2,
  Info,
  TriangleAlert,
} from "lucide-react";
import { Button } from "../Button";
import {
  Modal,
  type ModalCloseReason,
  type ModalPortalTarget,
} from "../Modal";
import styles from "./ConfirmDialog.module.css";

export type ConfirmDialogCloseReason = ModalCloseReason | "cancel-button";
export type ConfirmDialogType = "info" | "warning" | "error" | "success";
export type ConfirmDialogAction = () => void | Promise<void>;

interface ConfirmDialogContextValue {
  cancelLabel: ReactNode;
  confirmLabel: ReactNode;
}

const ConfirmDialogContext = createContext<ConfirmDialogContextValue>({
  cancelLabel: "Cancel",
  confirmLabel: "Confirm",
});

export interface ConfirmDialogProviderProps {
  cancelLabel?: ReactNode;
  children: ReactNode;
  confirmLabel?: ReactNode;
}

export function ConfirmDialogProvider({
  cancelLabel = "Cancel",
  children,
  confirmLabel = "Confirm",
}: ConfirmDialogProviderProps) {
  const value = useMemo(
    () => ({ cancelLabel, confirmLabel }),
    [cancelLabel, confirmLabel],
  );
  return (
    <ConfirmDialogContext.Provider value={value}>
      {children}
    </ConfirmDialogContext.Provider>
  );
}

export interface ConfirmDialogProps {
  cancelText?: ReactNode;
  closeOnEscape?: boolean;
  closeOnOverlayClick?: boolean;
  confirmDanger?: boolean;
  confirmText?: ReactNode;
  icon?: ReactNode | false;
  isOpen: boolean;
  message?: ReactNode;
  dialogClassName?: string;
  onCancel?: () => void;
  onActionError?: (error: unknown, action: "confirm" | "secondary") => void;
  onClose: (reason: ConfirmDialogCloseReason) => void;
  onConfirm: ConfirmDialogAction;
  onSecondary?: ConfirmDialogAction;
  portalContainer?: ModalPortalTarget;
  overlayClassName?: string;
  pendingAction?: "confirm" | "secondary" | null;
  portalled?: boolean;
  preventScroll?: boolean;
  preview?: ReactNode;
  previewMaxHeight?: number | string;
  secondaryText?: ReactNode;
  showCancel?: boolean;
  showCloseButton?: boolean;
  testId?: string;
  title: ReactNode;
  type?: ConfirmDialogType;
}

const defaultIcons: Record<ConfirmDialogType, ReactNode> = {
  error: <AlertCircle aria-hidden="true" />,
  info: <Info aria-hidden="true" />,
  success: <CheckCircle2 aria-hidden="true" />,
  warning: <TriangleAlert aria-hidden="true" />,
};

export const ConfirmDialog = forwardRef<HTMLDivElement, ConfirmDialogProps>(
  function ConfirmDialog({
    cancelText,
    closeOnEscape = true,
    closeOnOverlayClick = true,
    confirmDanger = false,
    confirmText,
    icon,
    isOpen,
    message,
    dialogClassName,
    onCancel,
    onActionError,
    onClose,
    onConfirm,
    onSecondary,
    portalContainer,
    overlayClassName,
    pendingAction: controlledPendingAction,
    portalled = true,
    preventScroll = true,
    preview,
    previewMaxHeight = 240,
    secondaryText,
    showCancel = true,
    showCloseButton = false,
    testId,
    title,
    type = "warning",
  }, ref) {
    const labels = useContext(ConfirmDialogContext);
    const confirmButtonRef = useRef<HTMLButtonElement>(null);
    const mountedRef = useRef(true);
    const [internalPendingAction, setInternalPendingAction] = useState<"confirm" | "secondary" | null>(null);
    const pendingAction = controlledPendingAction ?? internalPendingAction;
    const busy = pendingAction !== null;
    const resolvedIcon = icon === false ? null : icon ?? defaultIcons[type];
    const hasMessage = message !== undefined && message !== null && message !== "";
    const hasPreview = preview !== undefined && preview !== null && preview !== "";
    const resolvedCancelText = cancelText ?? labels.cancelLabel;
    const resolvedConfirmText = confirmText ?? labels.confirmLabel;

    useEffect(() => () => {
      mountedRef.current = false;
    }, []);

    useEffect(() => {
      if (!isOpen) setInternalPendingAction(null);
    }, [isOpen]);

    const runAction = useCallback(async (
      actionName: "confirm" | "secondary",
      action: ConfirmDialogAction | undefined,
    ) => {
      if (!action || busy) return;
      try {
        const result = action();
        if (result && typeof result.then === "function") {
          setInternalPendingAction(actionName);
          await result;
        }
      } catch (error) {
        onActionError?.(error, actionName);
      } finally {
        if (mountedRef.current) setInternalPendingAction(null);
      }
    }, [busy, onActionError]);

    const requestCancel = useCallback((reason: ConfirmDialogCloseReason) => {
      if (busy) return;
      onCancel?.();
      onClose(reason);
    }, [busy, onCancel, onClose]);

    return (
      <Modal
        closeOnEscape={!busy && closeOnEscape}
        closeOnOverlayClick={!busy && closeOnOverlayClick}
        contentPadding="lg"
        dialogClassName={dialogClassName}
        footer={(
          <>
            {showCancel && (
              <Button disabled={busy} onClick={() => requestCancel("cancel-button")} variant="fill">
                {resolvedCancelText}
              </Button>
            )}
            {secondaryText !== undefined && secondaryText !== null && (
              <Button
                disabled={busy}
                loading={pendingAction === "secondary"}
                onClick={() => void runAction("secondary", onSecondary)}
                variant="outline"
              >
                {secondaryText}
              </Button>
            )}
            <Button
              disabled={busy}
              loading={pendingAction === "confirm"}
              onClick={() => void runAction("confirm", onConfirm)}
              ref={confirmButtonRef}
              tone={confirmDanger || type === "error" ? "danger" : "neutral"}
              variant={confirmDanger || type === "error" ? "primary" : "fill"}
            >
              {resolvedConfirmText}
            </Button>
          </>
        )}
        initialFocusRef={confirmButtonRef}
        isOpen={isOpen}
        onClose={requestCancel}
        overlayClassName={overlayClassName}
        portalContainer={portalContainer}
        portalled={portalled}
        preventScroll={preventScroll}
        ref={ref}
        role="alertdialog"
        showCloseButton={showCloseButton}
        showScrollbar={false}
        size="small"
        testId={testId}
        title={title}
      >
        {(hasMessage || hasPreview) && (
          <div className={styles.content} data-bf-component="confirm-dialog" data-bf-part="content">
            {hasMessage && (
              <div
                className={styles.messageRow}
                data-bf-component="confirm-dialog"
                data-bf-part="messageRow"
              >
                {resolvedIcon !== null && (
                  <span
                    aria-hidden="true"
                    className={styles.icon}
                    data-bf-component="confirm-dialog"
                    data-bf-part="icon"
                    data-bf-status={type === "error" ? "danger" : type}
                  >
                    {resolvedIcon}
                  </span>
                )}
                <div
                  className={styles.message}
                  data-bf-component="confirm-dialog"
                  data-bf-part="message"
                >
                  {message}
                </div>
              </div>
            )}
            {hasPreview && (
              <div
                className={styles.preview}
                data-bf-component="confirm-dialog"
                data-bf-part="preview"
                style={{ maxBlockSize: previewMaxHeight }}
              >
                {typeof preview === "string" ? <pre>{preview}</pre> : preview}
              </div>
            )}
          </div>
        )}
      </Modal>
    );
  },
);
