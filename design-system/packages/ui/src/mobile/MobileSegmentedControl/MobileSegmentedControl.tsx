import { type HTMLAttributes, type ReactNode } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileSegmentedControl.module.css";
export interface MobileSegmentedControlOption<Value extends string = string> { disabled?: boolean; label: ReactNode; value: Value; }
export interface MobileSegmentedControlProps<Value extends string = string> extends Omit<HTMLAttributes<HTMLDivElement>, "onChange"> { "aria-label": string; onChange: (value: Value) => void; options: readonly MobileSegmentedControlOption<Value>[]; value: Value; }
export function MobileSegmentedControl<Value extends string>({ "aria-label": ariaLabel, className, onChange, options, value, ...props }: MobileSegmentedControlProps<Value>) {
  return <div {...props} aria-label={ariaLabel} className={classNames(styles.root, className)} data-openbitfun-component="mobile-segmented-control" role="radiogroup">{options.map(option => <button aria-checked={option.value === value} className={styles.option} data-selected={option.value === value ? "true" : "false"} disabled={option.disabled} key={option.value} onClick={() => onChange(option.value)} role="radio" type="button">{option.label}</button>)}</div>;
}
