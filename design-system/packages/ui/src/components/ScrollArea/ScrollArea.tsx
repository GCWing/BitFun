import { forwardRef, type HTMLAttributes } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./ScrollArea.module.css";

export type ScrollAreaOrientation = "vertical" | "horizontal" | "both";
export type ScrollbarVisibility = "auto" | "always" | "hidden";

export interface ScrollAreaProps extends HTMLAttributes<HTMLDivElement> {
  "data-openbitfun-component"?: string;
  "data-openbitfun-part"?: string;
  orientation?: ScrollAreaOrientation;
  scrollbarVisibility?: ScrollbarVisibility;
}

export const ScrollArea = forwardRef<HTMLDivElement, ScrollAreaProps>(
  function ScrollArea({
    className,
    "data-openbitfun-component": component = "scroll-area",
    "data-openbitfun-part": part = "viewport",
    orientation = "vertical",
    scrollbarVisibility = "auto",
    ...props
  }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.root, className)}
        data-openbitfun-component={component}
        data-openbitfun-orientation={orientation}
        data-openbitfun-part={part}
        data-openbitfun-scrollbar-visibility={scrollbarVisibility}
        ref={ref}
      />
    );
  },
);
