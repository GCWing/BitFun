import { forwardRef, type AnchorHTMLAttributes } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileLink.module.css";

export type MobileLinkAppearance = "inline" | "surface";

export interface MobileLinkProps extends AnchorHTMLAttributes<HTMLAnchorElement> {
  appearance?: MobileLinkAppearance;
}

export const MobileLink = forwardRef<HTMLAnchorElement, MobileLinkProps>(
  function MobileLink({ appearance = "inline", className, ...props }, ref) {
    return (
      <a
        {...props}
        className={classNames(styles.root, className)}
        data-appearance={appearance}
        data-openbitfun-component="mobile-link"
        ref={ref}
      />
    );
  },
);
