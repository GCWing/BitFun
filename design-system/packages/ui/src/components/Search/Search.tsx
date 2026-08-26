import {
  forwardRef,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Search.module.css";

export interface SearchProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "size" | "type"> {
  containerClassName?: string;
  leadingIcon?: ReactNode;
  trailingContent?: ReactNode;
  "data-bf-preview-state"?: "focus" | "hover";
}

function SearchGlyph() {
  return (
    <svg
      aria-hidden="true"
      fill="none"
      viewBox="0 0 24 24"
      xmlns="http://www.w3.org/2000/svg"
    >
      <circle cx="11" cy="11" r="7" stroke="currentColor" strokeWidth="2" />
      <path
        d="m16.5 16.5 4 4"
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth="2"
      />
    </svg>
  );
}

export const Search = forwardRef<HTMLInputElement, SearchProps>(function Search({
  className,
  containerClassName,
  disabled,
  leadingIcon,
  trailingContent,
  "data-bf-preview-state": previewState,
  ...props
}, ref) {
  return (
    <span
      className={classNames(styles.root, containerClassName)}
      data-bf-component="search"
      data-bf-part="root"
      data-bf-preview-state={previewState}
      data-disabled={disabled || undefined}
    >
      <span className={styles.content} data-bf-part="content">
        <span aria-hidden="true" className={styles.icon} data-bf-part="leadingIcon">
          {leadingIcon ?? <SearchGlyph />}
        </span>
        <input
          {...props}
          className={classNames(styles.input, className)}
          disabled={disabled}
          ref={ref}
          type="search"
        />
      </span>
      {trailingContent && (
        <span className={styles.trailing} data-bf-part="trailingContent">
          {trailingContent}
        </span>
      )}
    </span>
  );
});
