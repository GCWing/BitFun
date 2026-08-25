import {
  forwardRef,
  type ChangeEventHandler,
  type InputHTMLAttributes,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Switch.module.css";

export interface SwitchProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "children" | "type"> {
  onCheckedChange?: (checked: boolean) => void;
}

export const Switch = forwardRef<HTMLInputElement, SwitchProps>(function Switch({
  className,
  onChange,
  onCheckedChange,
  ...props
}, ref) {
  const handleChange: ChangeEventHandler<HTMLInputElement> = (event) => {
    onChange?.(event);
    onCheckedChange?.(event.currentTarget.checked);
  };

  return (
    <span
      className={classNames(styles.switch, className)}
      data-bf-component="switch"
    >
      <input
        {...props}
        className={styles.input}
        onChange={handleChange}
        ref={ref}
        role="switch"
        type="checkbox"
      />
      <span aria-hidden="true" className={styles.track}>
        <span className={styles.thumb} />
      </span>
    </span>
  );
});
