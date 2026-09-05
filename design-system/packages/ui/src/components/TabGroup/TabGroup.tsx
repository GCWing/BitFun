import {
  forwardRef,
  useId,
  useRef,
  useState,
  type HTMLAttributes,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./TabGroup.module.css";

export interface TabGroupItem {
  disabled?: boolean;
  endAction?: ReactNode;
  icon?: ReactNode;
  id?: string;
  label: ReactNode;
  panelId?: string;
  value: string;
}

export type TabGroupSize = "sm" | "md";

export interface TabGroupProps
  extends Omit<
    HTMLAttributes<HTMLDivElement>,
    "children" | "defaultValue" | "onChange"
  > {
  defaultValue?: string;
  items: readonly TabGroupItem[];
  onValueChange?: (value: string) => void;
  size?: TabGroupSize;
  value?: string;
}

function getFirstEnabledValue(items: readonly TabGroupItem[]): string {
  return items.find((item) => !item.disabled)?.value ?? "";
}

function getSelectedValue(
  items: readonly TabGroupItem[],
  candidate: string | undefined,
): string {
  const candidateItem = items.find((item) => item.value === candidate);
  return candidateItem && !candidateItem.disabled
    ? candidateItem.value
    : getFirstEnabledValue(items);
}

export const TabGroup = forwardRef<HTMLDivElement, TabGroupProps>(function TabGroup({
  className,
  defaultValue,
  items,
  onValueChange,
  size = "md",
  value,
  ...props
}, ref) {
  const generatedId = useId();
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const [uncontrolledValue, setUncontrolledValue] = useState(
    () => defaultValue ?? getFirstEnabledValue(items),
  );
  const selectedValue = getSelectedValue(items, value ?? uncontrolledValue);

  function selectItem(item: TabGroupItem) {
    if (item.disabled || item.value === selectedValue) {
      return;
    }
    if (value === undefined) {
      setUncontrolledValue(item.value);
    }
    onValueChange?.(item.value);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLButtonElement>, currentIndex: number) {
    const enabledItems = items
      .map((item, index) => ({ index, item }))
      .filter(({ item }) => !item.disabled);
    if (enabledItems.length === 0) {
      return;
    }

    const currentEnabledIndex = enabledItems.findIndex(({ index }) => index === currentIndex);
    const isRtl = event.currentTarget.ownerDocument.defaultView
      ?.getComputedStyle(event.currentTarget).direction === "rtl";
    let target: (typeof enabledItems)[number] | undefined;

    if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
      const movesForward = event.key === "ArrowRight" ? !isRtl : isRtl;
      const offset = movesForward ? 1 : -1;
      const nextIndex = (currentEnabledIndex + offset + enabledItems.length)
        % enabledItems.length;
      target = enabledItems[nextIndex];
    } else if (event.key === "Home") {
      target = enabledItems[0];
    } else if (event.key === "End") {
      target = enabledItems[enabledItems.length - 1];
    } else {
      return;
    }

    if (!target) {
      return;
    }
    event.preventDefault();
    selectItem(target.item);
    tabRefs.current[target.index]?.focus();
  }

  return (
    <div
      {...props}
      aria-orientation="horizontal"
      className={classNames(styles.tabGroup, className)}
      data-openbitfun-component="tab-group"
      data-openbitfun-part="root"
      data-size={size}
      ref={ref}
      role="tablist"
    >
      {items.map((item, index) => {
        const selected = item.value === selectedValue;
        const hasEndAction = item.endAction !== undefined && item.endAction !== null;
        const hasIcon = Boolean(item.icon);
        return (
          <div
            className={styles.item}
            data-openbitfun-part="item"
            data-has-end-action={hasEndAction ? "true" : "false"}
            data-has-icon={hasIcon ? "true" : "false"}
            key={item.value}
          >
            <button
              aria-controls={item.panelId}
              aria-disabled={item.disabled || undefined}
              aria-selected={selected}
              className={styles.tab}
              data-openbitfun-part="tab"
              data-openbitfun-value={item.value}
              disabled={item.disabled}
              id={item.id ?? `${generatedId}-tab-${index}`}
              onClick={() => selectItem(item)}
              onKeyDown={(event) => handleKeyDown(event, index)}
              ref={(node) => {
                tabRefs.current[index] = node;
              }}
              role="tab"
              tabIndex={selected ? 0 : -1}
              type="button"
            >
              {hasIcon && (
                <span aria-hidden="true" className={styles.icon} data-openbitfun-part="icon">
                  {item.icon}
                </span>
              )}
              <span className={styles.label} data-openbitfun-part="label">{item.label}</span>
            </button>
            {hasEndAction && (
              <span className={styles.endAction} data-openbitfun-part="endAction">
                {item.endAction}
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
});
