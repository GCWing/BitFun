import { forwardRef, type HTMLAttributes } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./ScrollArea.module.css";

export type ScrollAreaOrientation = "vertical" | "horizontal" | "both";
export type ScrollbarVisibility = "auto" | "always" | "hidden";

export interface ScrollAreaProps extends HTMLAttributes<HTMLDivElement> {
  orientation?: ScrollAreaOrientation;
  scrollbarVisibility?: ScrollbarVisibility;
}

export const ScrollArea = forwardRef<HTMLDivElement, ScrollAreaProps>(
  function ScrollArea({
    className,
    orientation = "vertical",
    scrollbarVisibility = "auto",
    ...props
  }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.root, className)}
        data-bf-component="scroll-area"
        data-bf-orientation={orientation}
        data-bf-part="viewport"
        data-bf-scrollbar-visibility={scrollbarVisibility}
        ref={ref}
      />
    );
  },
);
