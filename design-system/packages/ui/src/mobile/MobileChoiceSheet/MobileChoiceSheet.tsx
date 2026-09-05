import type { ReactNode } from "react";
import { classNames } from "../../internal/classNames";
import { MobileButton } from "../MobileButton";
import {
  MobileListRow,
  type MobileListRowAppearance,
} from "../MobileListRow";
import {
  MobileSheet,
  type MobileSheetProps,
} from "../MobileSheet";
import styles from "./MobileChoiceSheet.module.css";

export interface MobileChoiceSheetOption {
  className?: string;
  description?: ReactNode;
  disabled?: boolean;
  label: ReactNode;
  leading?: ReactNode;
  trailing?: ReactNode;
  value: string;
}

export interface MobileChoiceSheetProps
  extends Omit<MobileSheetProps, "children" | "footer" | "onSelect"> {
  cancelLabel?: ReactNode;
  emptyContent?: ReactNode;
  onSelect: (value: string) => void;
  optionAppearance?: MobileListRowAppearance;
  options: readonly MobileChoiceSheetOption[];
  selectedValue?: string;
}

/** A mobile bottom sheet for short, mutually exclusive product choices. */
export function MobileChoiceSheet({
  cancelLabel,
  className,
  emptyContent,
  onOpenChange,
  onSelect,
  optionAppearance = "surface",
  options,
  selectedValue,
  ...sheetProps
}: MobileChoiceSheetProps) {
  return (
    <MobileSheet
      {...sheetProps}
      className={classNames(styles.sheet, className)}
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
      <div className={styles.options} data-openbitfun-component="mobile-choice-sheet" data-openbitfun-part="options" role="radiogroup">
        {options.length === 0 ? emptyContent : options.map((option) => (
          <MobileListRow
            appearance={optionAppearance}
            className={option.className}
            disabled={option.disabled}
            key={option.value}
            label={option.label}
            leading={option.leading}
            onClick={() => onSelect(option.value)}
            role="radio"
            selected={option.value === selectedValue}
            supportingText={option.description}
            trailing={option.trailing}
          />
        ))}
      </div>
    </MobileSheet>
  );
}
