import {
  forwardRef,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileListRow.module.css";

export type MobileListRowAppearance = "plain" | "surface";
export type MobileListRowTone = "neutral" | "danger";

export interface MobileListRowProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  appearance?: MobileListRowAppearance;
  label: ReactNode;
  leading?: ReactNode;
  selected?: boolean;
  supportingText?: ReactNode;
  tone?: MobileListRowTone;
  trailing?: ReactNode;
}

export const MobileListRow = forwardRef<HTMLButtonElement, MobileListRowProps>(
  function MobileListRow({
    appearance = "plain",
    className,
    label,
    leading,
    role,
    selected = false,
    supportingText,
    tone = "neutral",
    trailing,
    type = "button",
    ...props
  }, ref) {
    return (
      <button
        {...props}
        aria-checked={role === "radio" ? selected : undefined}
        aria-current={selected && role !== "radio" ? "true" : undefined}
        className={classNames(styles.root, className)}
        data-appearance={appearance}
        data-bf-component="mobile-list-row"
        data-selected={selected ? "true" : "false"}
        data-tone={tone}
        ref={ref}
        role={role}
        type={type}
      >
        {leading !== undefined && leading !== null && (
          <span className={styles.leading} data-bf-part="leading">{leading}</span>
        )}
        <span className={styles.copy} data-bf-part="copy">
          <span className={styles.label} data-bf-part="label">{label}</span>
          {supportingText !== undefined && supportingText !== null && (
            <span className={styles.supportingText} data-bf-part="supporting-text">
              {supportingText}
            </span>
          )}
        </span>
        {trailing !== undefined && trailing !== null && (
          <span className={styles.trailing} data-bf-part="trailing">{trailing}</span>
        )}
      </button>
    );
  },
);
