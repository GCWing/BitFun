import { forwardRef, type HTMLAttributes } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./ScrollArea.module.css";

export type ScrollAreaOrientation = "vertical" | "horizontal" | "both";
export type ScrollbarVisibility = "auto" | "always" | "hidden";

export interface ScrollAreaProps extends HTMLAttributes<HTMLDivElement> {
  "data-bf-component"?: string;
  "data-bf-part"?: string;
  orientation?: ScrollAreaOrientation;
  scrollbarVisibility?: ScrollbarVisibility;
}

export const ScrollArea = forwardRef<HTMLDivElement, ScrollAreaProps>(
  function ScrollArea({
    className,
    "data-bf-component": component = "scroll-area",
    "data-bf-part": part = "viewport",
    orientation = "vertical",
    scrollbarVisibility = "auto",
    ...props
  }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.root, className)}
        data-bf-component={component}
        data-bf-orientation={orientation}
        data-bf-part={part}
        data-bf-scrollbar-visibility={scrollbarVisibility}
        ref={ref}
      />
    );
  },
);
