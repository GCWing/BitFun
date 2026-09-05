import { forwardRef, type HTMLAttributes } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileCard.module.css";

export type MobileCardAppearance = "plain" | "surface" | "elevated";
export type MobileCardPadding = "none" | "sm" | "md" | "lg";

export interface MobileCardProps extends HTMLAttributes<HTMLDivElement> {
  appearance?: MobileCardAppearance;
  padding?: MobileCardPadding;
}

export const MobileCard = forwardRef<HTMLDivElement, MobileCardProps>(function MobileCard({ appearance = "surface", className, padding = "md", ...props }, ref) {
  return <div {...props} className={classNames(styles.root, className)} data-appearance={appearance} data-openbitfun-component="mobile-card" data-padding={padding} ref={ref} />;
});
