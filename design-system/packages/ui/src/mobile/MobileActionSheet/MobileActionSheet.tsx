import type { ReactNode } from "react";
import { MobileButton } from "../MobileButton";
import { MobileListRow } from "../MobileListRow";
import { MobileSheet, type MobileSheetProps } from "../MobileSheet";
import styles from "./MobileActionSheet.module.css";

export interface MobileActionSheetItem {
  description?: ReactNode;
  disabled?: boolean;
  id: string;
  label: ReactNode;
  leading?: ReactNode;
  tone?: "neutral" | "danger";
  trailing?: ReactNode;
}

export interface MobileActionSheetProps
  extends Omit<MobileSheetProps, "children" | "footer"> {
  actions: readonly MobileActionSheetItem[];
  cancelLabel?: ReactNode;
  closeOnAction?: boolean;
  onAction: (id: string) => void;
}

/** A consistent mobile action list presented inside a dismissible sheet. */
export function MobileActionSheet({
  actions,
  cancelLabel,
  closeOnAction = true,
  onAction,
  onOpenChange,
  ...sheetProps
}: MobileActionSheetProps) {
  const activate = (id: string) => {
    onAction(id);
    if (closeOnAction) onOpenChange(false, "programmatic");
  };

  return (
    <MobileSheet
      {...sheetProps}
      footer={cancelLabel !== undefined && cancelLabel !== null ? (
        <MobileButton
          appearance="plain"
          block
          className={styles.cancel}
          onClick={() => onOpenChange(false, "programmatic")}
        >
          {cancelLabel}
        </MobileButton>
      ) : undefined}
      onOpenChange={onOpenChange}
    >
      <div className={styles.actions} data-openbitfun-component="mobile-action-sheet" data-openbitfun-part="actions">
        {actions.map((action) => (
          <MobileListRow
            appearance="plain"
            disabled={action.disabled}
            key={action.id}
            label={action.label}
            leading={action.leading}
            onClick={() => activate(action.id)}
            supportingText={action.description}
            tone={action.tone}
            trailing={action.trailing}
          />
        ))}
      </div>
    </MobileSheet>
  );
}
