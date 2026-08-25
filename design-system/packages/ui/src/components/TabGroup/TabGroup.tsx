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

export interface TabGroupProps
  extends Omit<
    HTMLAttributes<HTMLDivElement>,
    "children" | "defaultValue" | "onChange"
  > {
  defaultValue?: string;
  items: readonly TabGroupItem[];
  onValueChange?: (value: string) => void;
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
      data-bf-component="tab-group"
      data-bf-part="root"
      ref={ref}
      role="tablist"
    >
      {items.map((item, index) => {
        const selected = item.value === selectedValue;
        const hasEndAction = item.endAction !== undefined && item.endAction !== null;
        return (
          <div
            className={styles.item}
            data-bf-part="item"
            data-has-end-action={hasEndAction ? "true" : "false"}
            key={item.value}
          >
            <button
              aria-controls={item.panelId}
              aria-disabled={item.disabled || undefined}
              aria-selected={selected}
              className={styles.tab}
              data-bf-part="tab"
              data-bf-value={item.value}
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
              {item.icon && (
                <span aria-hidden="true" className={styles.icon} data-bf-part="icon">
                  {item.icon}
                </span>
              )}
              <span className={styles.label} data-bf-part="label">{item.label}</span>
            </button>
            {hasEndAction && (
              <span className={styles.endAction} data-bf-part="endAction">
                {item.endAction}
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
});
