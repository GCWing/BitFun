import {
  forwardRef,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileTextField.module.css";

export type MobileTextFieldAppearance = "soft" | "surface";

export interface MobileTextFieldProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  appearance?: MobileTextFieldAppearance;
  inputClassName?: string;
  invalid?: boolean;
  leading?: ReactNode;
  trailing?: ReactNode;
}

export const MobileTextField = forwardRef<HTMLInputElement, MobileTextFieldProps>(
  function MobileTextField({
    appearance = "soft",
    "aria-invalid": ariaInvalid,
    className,
    disabled,
    inputClassName,
    invalid = false,
    leading,
    trailing,
    type = "text",
    ...props
  }, ref) {
    const resolvedInvalid = invalid || ariaInvalid === true || ariaInvalid === "true";

    return (
      <div
        className={classNames(styles.root, className)}
        data-appearance={appearance}
        data-openbitfun-component="mobile-text-field"
        data-disabled={disabled ? "true" : "false"}
        data-invalid={resolvedInvalid ? "true" : "false"}
      >
        {leading !== undefined && leading !== null && (
          <span aria-hidden="true" className={styles.slot} data-openbitfun-part="leading">
            {leading}
          </span>
        )}
        <input
          {...props}
          aria-invalid={resolvedInvalid || undefined}
          className={classNames(styles.input, inputClassName)}
          disabled={disabled}
          ref={ref}
          type={type}
        />
        {trailing !== undefined && trailing !== null && (
          <span className={styles.slot} data-openbitfun-part="trailing">
            {trailing}
          </span>
        )}
      </div>
    );
  },
);
