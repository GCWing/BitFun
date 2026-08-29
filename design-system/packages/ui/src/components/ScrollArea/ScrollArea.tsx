import { forwardRef, type HTMLAttributes } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./ScrollArea.module.css";

export type ScrollAreaOrientation = "vertical" | "horizontal" | "both";
export type ScrollbarVisibility = "auto" | "always" | "hidden";

export interface ScrollAreaProps extends HTMLAttributes<HTMLDivElement> {
  orientation?: ScrollAreaOrientation;
  scrollbarVisibility?: ScrollbarVisibility;
  "data-bf-component"?: string;
  "data-bf-part"?: string;
}

export const ScrollArea = forwardRef<HTMLDivElement, ScrollAreaProps>(
  function ScrollArea({
    className,
    orientation = "vertical",
    scrollbarVisibility = "auto",
    "data-bf-component": dataBfComponent = "scroll-area",
    "data-bf-part": dataBfPart = "viewport",
    ...props
  }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.root, className)}
        data-bf-component={dataBfComponent}
        data-bf-orientation={orientation}
        data-bf-part={dataBfPart}
        data-bf-scrollbar-visibility={scrollbarVisibility}
        ref={ref}
      />
    );
  },
);
