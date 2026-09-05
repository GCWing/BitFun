import type { KeyboardEvent, ReactNode } from "react";
import { MobileButton } from "../MobileButton";
import { MobileSheet, type MobileSheetProps } from "../MobileSheet";
import styles from "./MobileConfirmSheet.module.css";

export interface MobileConfirmSheetProps
  extends Omit<MobileSheetProps, "children" | "footer"> {
  cancelLabel: ReactNode;
  children?: ReactNode;
  confirmDisabled?: boolean;
  confirmLabel: ReactNode;
  confirmTone?: "primary" | "danger";
  icon?: ReactNode;
  pending?: boolean;
  onConfirm: () => void;
}

/** A mobile confirmation sheet with consistent pending and destructive behavior. */
export function MobileConfirmSheet({
  cancelLabel,
  children,
  confirmDisabled = false,
  confirmLabel,
  confirmTone = "primary",
  icon,
  onConfirm,
  onKeyDown,
  onOpenChange,
  pending = false,
  ...sheetProps
}: MobileConfirmSheetProps) {
  const close = () => {
    if (!pending) onOpenChange(false, "programmatic");
  };
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    onKeyDown?.(event);
    if (event.defaultPrevented || pending || confirmDisabled) return;
    if ((event.target as Element).closest("button, input, textarea, select, a[href]")) return;
    if (event.key === "Enter") {
      event.preventDefault();
      onConfirm();
    }
  };

  return (
    <MobileSheet
      {...sheetProps}
      footer={(
        <div className={styles.actions} data-openbitfun-part="actions">
          <MobileButton autoFocus disabled={pending} onClick={close}>{cancelLabel}</MobileButton>
          <MobileButton
            appearance={confirmTone}
            disabled={confirmDisabled || pending}
            loading={pending}
            onClick={onConfirm}
          >
            {confirmLabel}
          </MobileButton>
        </div>
      )}
      onKeyDown={handleKeyDown}
      onOpenChange={() => close()}
    >
      <div className={styles.content} data-openbitfun-component="mobile-confirm-sheet" data-openbitfun-part="content">
        {icon !== undefined && icon !== null && (
          <span aria-hidden="true" className={styles.icon} data-openbitfun-part="icon">{icon}</span>
        )}
        {children}
      </div>
    </MobileSheet>
  );
}
