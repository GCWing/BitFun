import {
  forwardRef,
  type KeyboardEventHandler,
  type ReactNode,
} from "react";
import { Input, type InputProps } from "../Input";
import { classNames } from "../../internal/classNames";
import styles from "./SearchField.module.css";

export interface SearchFieldProps
  extends Omit<InputProps, "leading" | "trailing" | "type"> {
  leadingIcon?: ReactNode;
  onSearch?: (value: string) => void;
  shortcut?: ReactNode;
}

export const SearchField = forwardRef<HTMLInputElement, SearchFieldProps>(function SearchField({
  className,
  leadingIcon,
  onKeyDown,
  onSearch,
  shortcut,
  ...props
}, ref) {
  const handleKeyDown: KeyboardEventHandler<HTMLInputElement> = (event) => {
    onKeyDown?.(event);
    if (!event.defaultPrevented && event.key === "Enter") {
      onSearch?.(event.currentTarget.value);
    }
  };

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
        trailing={shortcut === undefined ? undefined : (
          <span aria-hidden="true" className={styles.shortcut}>{shortcut}</span>
        )}
        type="search"
      />
    </span>
  );
});
