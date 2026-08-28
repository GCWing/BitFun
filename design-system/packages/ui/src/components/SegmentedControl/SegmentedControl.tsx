import {
  forwardRef,
  useRef,
  useState,
  type HTMLAttributes,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./SegmentedControl.module.css";

export interface SegmentedControlOption {
  disabled?: boolean;
  icon?: ReactNode;
  label: ReactNode;
  value: string;
}

export interface SegmentedControlProps
  extends Omit<
    HTMLAttributes<HTMLDivElement>,
    "children" | "defaultValue" | "onChange"
  > {
  defaultValue?: string;
  disabled?: boolean;
  onValueChange?: (value: string) => void;
  options: readonly SegmentedControlOption[];
  value?: string;
}

function getFirstEnabledValue(options: readonly SegmentedControlOption[]): string {
  return options.find((option) => !option.disabled)?.value ?? "";
}

function getSelectedValue(
  options: readonly SegmentedControlOption[],
  candidate: string | undefined,
): string {
  const candidateOption = options.find((option) => option.value === candidate);
  return candidateOption && !candidateOption.disabled
    ? candidateOption.value
    : getFirstEnabledValue(options);
}

export const SegmentedControl = forwardRef<HTMLDivElement, SegmentedControlProps>(
  function SegmentedControl({
    className,
    defaultValue,
    disabled = false,
    onValueChange,
    options,
    value,
    ...props
  }, ref) {
    const segmentRefs = useRef<Array<HTMLButtonElement | null>>([]);
    const [uncontrolledValue, setUncontrolledValue] = useState(
      () => defaultValue ?? getFirstEnabledValue(options),
    );
    const selectedValue = getSelectedValue(options, value ?? uncontrolledValue);

    function selectOption(option: SegmentedControlOption) {
      if (disabled || option.disabled || option.value === selectedValue) {
        return;
      }
      if (value === undefined) {
        setUncontrolledValue(option.value);
      }
      onValueChange?.(option.value);
    }

    function handleKeyDown(
      event: KeyboardEvent<HTMLButtonElement>,
      currentIndex: number,
    ) {
      const enabledOptions = options
        .map((option, index) => ({ index, option }))
        .filter(({ option }) => !option.disabled);
      if (enabledOptions.length === 0) {
        return;
      }

      const currentEnabledIndex = enabledOptions.findIndex(
        ({ index }) => index === currentIndex,
      );
      const isRtl = event.currentTarget.ownerDocument.defaultView
        ?.getComputedStyle(event.currentTarget).direction === "rtl";
      let target: (typeof enabledOptions)[number] | undefined;

      if (
        event.key === "ArrowRight"
        || event.key === "ArrowLeft"
        || event.key === "ArrowDown"
        || event.key === "ArrowUp"
      ) {
        const movesForward = event.key === "ArrowDown"
          || (event.key === "ArrowRight" ? !isRtl : event.key === "ArrowLeft" && isRtl);
        const offset = movesForward ? 1 : -1;
        const nextIndex = (currentEnabledIndex + offset + enabledOptions.length)
          % enabledOptions.length;
        target = enabledOptions[nextIndex];
      } else if (event.key === "Home") {
        target = enabledOptions[0];
      } else if (event.key === "End") {
        target = enabledOptions[enabledOptions.length - 1];
      } else {
        return;
      }

      if (!target) {
        return;
      }
      event.preventDefault();
      selectOption(target.option);
      segmentRefs.current[target.index]?.focus();
    }

    return (
      <div
        {...props}
        aria-disabled={disabled || undefined}
        className={classNames(styles.root, className)}
        data-bf-component="segmented-control"
        data-bf-part="root"
        data-disabled={disabled ? "true" : "false"}
        ref={ref}
        role="radiogroup"
      >
        {options.map((option, index) => {
          const selected = option.value === selectedValue;
          return (
            <button
              aria-checked={selected}
              className={styles.segment}
              data-bf-part="segment"
              data-bf-value={option.value}
              disabled={disabled || option.disabled}
              key={option.value}
              onClick={() => selectOption(option)}
              onKeyDown={(event) => handleKeyDown(event, index)}
              ref={(node) => {
                segmentRefs.current[index] = node;
              }}
              role="radio"
              tabIndex={selected ? 0 : -1}
              type="button"
            >
              {option.icon && (
                <span aria-hidden="true" className={styles.icon} data-bf-part="icon">
                  {option.icon}
                </span>
              )}
              <span className={styles.label} data-bf-part="label">{option.label}</span>
            </button>
          );
        })}
      </div>
    );
  },
);
