import {
  forwardRef,
  useCallback,
  useId,
  useRef,
  useState,
  type ChangeEventHandler,
  type TextareaHTMLAttributes,
} from "react";
import { classNames } from "../../internal/classNames";
import { isImeOwnedKeyboardEvent } from "../../internal/ime";
import styles from "./Textarea.module.css";

export interface TextareaProps extends Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "className"> {
  autoResize?: boolean;
  className?: string;
  error?: boolean;
  errorMessage?: string;
  hint?: string;
  invalid?: boolean;
  label?: string;
  onValueChange?: (value: string) => void;
  showCount?: boolean;
  textareaClassName?: string;
  variant?: "default" | "filled" | "outlined";
}

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(function Textarea({
  "aria-describedby": ariaDescribedBy,
  "aria-invalid": ariaInvalid,
  autoResize = false,
  className,
  error = false,
  errorMessage,
  hint,
  id,
  invalid = false,
  label,
  maxLength,
  onChange,
  onCompositionEnd,
  onCompositionStart,
  onKeyDown,
  onValueChange,
  required,
  showCount = false,
  textareaClassName,
  value,
  variant = "default",
  ...props
}, ref) {
  const generatedId = useId();
  const resolvedId = id ?? `${generatedId}-textarea`;
  const supportId = `${generatedId}-support`;
  const compositionActiveRef = useRef(false);
  const [uncontrolledCount, setUncontrolledCount] = useState(() => String(props.defaultValue ?? "").length);
  const count = value === undefined ? uncontrolledCount : String(value).length;
  const resolvedInvalid = invalid || error || (ariaInvalid !== undefined && ariaInvalid !== false && ariaInvalid !== "false");
  const hasSupport = Boolean((resolvedInvalid && errorMessage) || (!resolvedInvalid && hint) || showCount);

  const resize = useCallback((node: HTMLTextAreaElement) => {
    if (!autoResize) return;
    node.style.height = "auto";
    node.style.height = `${node.scrollHeight}px`;
  }, [autoResize]);

  const handleChange: ChangeEventHandler<HTMLTextAreaElement> = (event) => {
    setUncontrolledCount(event.currentTarget.value.length);
    resize(event.currentTarget);
    onChange?.(event);
    onValueChange?.(event.currentTarget.value);
  };

  return (
    <span
      className={classNames(styles.root, className)}
      data-auto-resize={autoResize ? "true" : "false"}
      data-bf-component="textarea"
      data-invalid={resolvedInvalid ? "true" : "false"}
      data-variant={variant}
    >
      {label && <label className={styles.label} htmlFor={resolvedId}>{label}{required && <span className={styles.required}>*</span>}</label>}
      <textarea
        {...props}
        aria-describedby={hasSupport ? supportId : ariaDescribedBy}
        aria-invalid={resolvedInvalid || undefined}
        className={classNames(styles.textarea, textareaClassName)}
        id={resolvedId}
        maxLength={maxLength}
        onChange={handleChange}
        onCompositionEnd={(event) => { compositionActiveRef.current = false; onCompositionEnd?.(event); }}
        onCompositionStart={(event) => { compositionActiveRef.current = true; onCompositionStart?.(event); }}
        onKeyDown={(event) => {
          if ((event.key === "Enter" || event.key === "Escape") && isImeOwnedKeyboardEvent(event, compositionActiveRef.current)) {
            event.stopPropagation();
            return;
          }
          onKeyDown?.(event);
        }}
        ref={ref}
        required={required}
        value={value}
      />
      {hasSupport && (
        <span className={styles.support} id={supportId}>
          <span className={resolvedInvalid ? styles.error : styles.hint}>{resolvedInvalid ? errorMessage : hint}</span>
          {showCount && <span className={styles.count}>{count}{maxLength ? ` / ${maxLength}` : ""}</span>}
        </span>
      )}
    </span>
  );
});
