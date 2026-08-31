import type { CSSProperties, HTMLAttributes } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Stack.module.css";

type StackGap = "0" | "1" | "2" | "3" | "4" | "5" | "6" | "8" | "10" | "12";

const stackGapVariables = {
  "0": "var(--bf-space-0)",
  "1": "var(--bf-space-1)",
  "2": "var(--bf-space-2)",
  "3": "var(--bf-space-3)",
  "4": "var(--bf-space-4)",
  "5": "var(--bf-space-5)",
  "6": "var(--bf-space-6)",
  "8": "var(--bf-space-8)",
  "10": "var(--bf-space-10)",
  "12": "var(--bf-space-12)",
} as const satisfies Record<StackGap, string>;

export interface StackProps extends HTMLAttributes<HTMLDivElement> {
  align?: "start" | "center" | "end" | "stretch";
  direction?: "horizontal" | "vertical";
  gap?: StackGap;
  justify?: "start" | "center" | "end" | "between";
  wrap?: boolean;
}

export function Stack({
  align = "stretch",
  className,
  direction = "vertical",
  gap = "3",
  justify = "start",
  style,
  wrap = false,
  ...props
}: StackProps) {
  const stackStyle = {
    ...style,
    "--_stack-gap": stackGapVariables[gap],
  } as CSSProperties;

  return (
    <div
      {...props}
      className={classNames(styles.stack, className)}
      data-align={align}
      data-direction={direction}
      data-justify={justify}
      data-wrap={wrap ? "true" : "false"}
      style={stackStyle}
    />
  );
}
