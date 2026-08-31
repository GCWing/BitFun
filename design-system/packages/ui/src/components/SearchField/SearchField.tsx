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
  clearLabel?: string;
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
  leadingIcon,
  onClear,
  onKeyDown,
  onSearch,
  shortcut,
  trailing,
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
          icon={<Icon name="xmark" />}
          onClick={onClear}
          size="sm"
          variant="quiet"
        />
      )
    : undefined;
  const endAdornment = clearAction ?? (shortcut === undefined ? undefined : (
    <span aria-hidden="true" className={styles.shortcut}>{shortcut}</span>
  ));
  const trailingContent = trailing === undefined && endAdornment === undefined
    ? undefined
    : (
        <>
          {trailing}
          {endAdornment}
        </>
      );

  return (
    <span className={classNames(styles.root, className)} data-bf-component="search-field">
      <Input
        {...props}
        className={styles.field}
        leading={leadingIcon === undefined ? undefined : (
          <span aria-hidden="true" className={styles.icon}>{leadingIcon}</span>
        )}
        onKeyDown={handleKeyDown}
        ref={ref}
        trailing={trailingContent}
        type="search"
      />
    </span>
  );
});
