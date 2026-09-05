import { forwardRef, type ButtonHTMLAttributes } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileScrim.module.css";
export interface MobileScrimProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  "aria-label": string;
  visible?: boolean;
}

export const MobileScrim = forwardRef<HTMLButtonElement, MobileScrimProps>(
  function MobileScrim({ className, type = "button", visible = true, ...props }, ref) {
    return (
      <button
        {...props}
        className={classNames(styles.root, className)}
        data-openbitfun-component="mobile-scrim"
        data-visible={visible ? "true" : "false"}
        ref={ref}
        type={type}
      />
    );
  },
);
