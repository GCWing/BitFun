import {
  forwardRef,
  type KeyboardEventHandler,
  type MouseEventHandler,
  type ReactNode,
} from "react";
import { Icon } from "../Icon";
import { IconButton } from "../IconButton";
import { Input, type InputProps } from "../Input";
import { classNames } from "../../internal/classNames";
import styles from "./SearchField.module.css";

export interface SearchFieldProps
  extends Omit<InputProps, "leading" | "trailing" | "type"> {
  /** Embedded delegates the field surface to its container; panel joins the input and footer in a frosted surface. */
  variant?: "default" | "embedded" | "panel";
  clearLabel?: string;
  /** Panel-only second row for caller-owned status and actions; search logic stays with the caller. */
  footer?: ReactNode;
  leadingIcon?: ReactNode;
  onClear?: MouseEventHandler<HTMLButtonElement>;
  onSearch?: (value: string) => void;
  shortcut?: ReactNode;
  /** Custom inline content before the clear action, e.g. match counts or a busy indicator. */
  trailing?: ReactNode;
}

export const SearchField = forwardRef<HTMLInputElement, SearchFieldProps>(function SearchField({
  className,
  clearLabel,
  disabled,
  footer,
  leadingIcon,
  onClear,
  onKeyDown,
  onSearch,
  readOnly,
  shortcut,
  trailing,
  variant = "default",
  ...props
}, ref) {
  const handleKeyDown: KeyboardEventHandler<HTMLInputElement> = (event) => {
    onKeyDown?.(event);
    if (!event.defaultPrevented && event.key === "Enter") {
      onSearch?.(event.currentTarget.value);
    }
  };
  const clearAction = clearLabel && onClear
    ? (
        <IconButton
          aria-label={clearLabel}
          className={styles.clear}
          disabled={disabled || readOnly}
          icon={<Icon name="xmark" />}
          onClick={(event) => {
            if (!disabled && !readOnly) onClear(event);
          }}
          onMouseDown={(event) => event.preventDefault()}
          shape="circle"
          size="xs"
          variant="quiet"
        />
      )
    : undefined;
  const shortcutHint = shortcut === undefined ? undefined : (
    <span aria-hidden="true" className={styles.shortcut}>{shortcut}</span>
  );
  const trailingContent = trailing === undefined && shortcutHint === undefined && clearAction === undefined
    ? undefined
    : (
        <>
          {trailing}
          {shortcutHint}
          {clearAction}
        </>
      );

  return (
    <span className={classNames(styles.root, className)} data-openbitfun-component="search-field" data-variant={variant}>
      <Input
        {...props}
        className={styles.field}
        disabled={disabled}
        leading={leadingIcon === undefined ? undefined : (
          <span aria-hidden="true" className={styles.icon} data-openbitfun-part="icon">{leadingIcon}</span>
        )}
        onKeyDown={handleKeyDown}
        ref={ref}
        readOnly={readOnly}
        trailing={trailingContent}
        type="search"
      />
      {variant === "panel" && footer != null && (
        <span className={styles.footer} data-openbitfun-part="footer">{footer}</span>
      )}
    </span>
  );
});
