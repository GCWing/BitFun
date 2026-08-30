import {
  forwardRef,
  useCallback,
  useEffect,
  useRef,
  useState,
  type InputHTMLAttributes,
} from "react";
import { classNames } from "../../internal/classNames";
import { isImeOwnedKeyboardEvent } from "../../internal/ime";
import styles from "./NumberInput.module.css";

export interface NumberInputProps extends Pick<InputHTMLAttributes<HTMLInputElement>, "id" | "aria-label" | "aria-labelledby" | "aria-describedby" | "aria-invalid" | "required"> {
  className?: string;
  decrementLabel?: string;
  disabled?: boolean;
  disableWheel?: boolean;
  draggable?: boolean;
  incrementLabel?: string;
  inputProps?: Omit<
    InputHTMLAttributes<HTMLInputElement>,
    "className" | "defaultValue" | "disabled" | "onChange" | "type" | "value"
  > & Record<`data-${string}`, string | number | boolean | undefined>;
  label?: string;
  max?: number;
  min?: number;
  onChange: (value: number) => void;
  precision?: number;
  showButtons?: boolean;
  size?: "small" | "medium" | "large" | "sm" | "md" | "lg";
  step?: number;
  unit?: string;
  value: number;
  variant?: "default" | "compact" | "stepper";
}

export const NumberInput = forwardRef<HTMLInputElement, NumberInputProps>(function NumberInput({
  className,
  decrementLabel = "Decrease value",
  disabled = false,
  disableWheel = false,
  incrementLabel = "Increase value",
  inputProps,
  label,
  max = Number.POSITIVE_INFINITY,
  min = Number.NEGATIVE_INFINITY,
  onChange,
  precision = 0,
  showButtons = true,
  size = "medium",
  step = 1,
  unit,
  value,
  variant = "default",
  id,
  required,
  "aria-label": ariaLabel,
  "aria-labelledby": ariaLabelledBy,
  "aria-describedby": ariaDescribedBy,
  "aria-invalid": ariaInvalid,
}, ref) {
  const format = useCallback((next: number) => precision > 0 ? next.toFixed(precision) : String(Math.round(next)), [precision]);
  const clamp = useCallback((next: number) => Math.min(max, Math.max(min, next)), [max, min]);
  const [draft, setDraft] = useState(() => format(value));
  const [editing, setEditing] = useState(false);
  const compositionActiveRef = useRef(false);

  useEffect(() => { if (!editing) setDraft(format(value)); }, [editing, format, value]);

  const commit = useCallback(() => {
    const parsed = Number.parseFloat(draft);
    if (Number.isFinite(parsed)) {
      const next = clamp(parsed);
      onChange(next);
      setDraft(format(next));
    } else {
      setDraft(format(value));
    }
    setEditing(false);
  }, [clamp, draft, format, onChange, value]);
  const changeBy = (amount: number) => onChange(clamp(value + amount));
  const normalizedSize = size === "small" ? "sm" : size === "large" ? "lg" : size === "medium" ? "md" : size;

  return (
    <span className={classNames(styles.root, className)} data-bf-component="number-input" data-disabled={disabled ? "true" : "false"} data-size={normalizedSize} data-variant={variant}>
      {label && <span className={styles.label} data-bf-part="label">{label}</span>}
      <span
        className={styles.control}
        data-bf-part="control"
        onWheel={(event) => {
          if (disabled || disableWheel || document.activeElement !== event.currentTarget.querySelector("input")) return;
          event.preventDefault();
          changeBy(event.deltaY < 0 ? step : -step);
        }}
      >
        <input
          id={id}
          required={required}
          aria-labelledby={ariaLabelledBy}
          aria-describedby={ariaDescribedBy}
          aria-invalid={ariaInvalid}
          {...inputProps}
          aria-label={inputProps?.["aria-label"] ?? ariaLabel ?? label}
          className={styles.input}
          data-bf-part="input"
          disabled={disabled}
          inputMode="decimal"
          onBlur={(event) => {
            inputProps?.onBlur?.(event);
            if (!event.defaultPrevented) commit();
          }}
          onChange={(event) => setDraft(event.currentTarget.value)}
          onCompositionEnd={(event) => {
            compositionActiveRef.current = false;
            inputProps?.onCompositionEnd?.(event);
          }}
          onCompositionStart={(event) => {
            compositionActiveRef.current = true;
            inputProps?.onCompositionStart?.(event);
          }}
          onFocus={(event) => {
            setEditing(true);
            inputProps?.onFocus?.(event);
          }}
          onKeyDown={(event) => {
            inputProps?.onKeyDown?.(event);
            if (event.defaultPrevented) return;
            if ((event.key === "Enter" || event.key === "Escape") && isImeOwnedKeyboardEvent(event, compositionActiveRef.current)) { event.stopPropagation(); return; }
            if (event.key === "ArrowUp") { event.preventDefault(); changeBy(step); }
            if (event.key === "ArrowDown") { event.preventDefault(); changeBy(-step); }
            if (event.key === "Enter") { commit(); event.currentTarget.blur(); }
            if (event.key === "Escape") { setDraft(format(value)); setEditing(false); event.currentTarget.blur(); }
          }}
          ref={ref}
          type="text"
          value={draft}
        />
        {unit && <span className={styles.unit} data-bf-part="unit">{unit}</span>}
        {showButtons && variant !== "compact" && (
          <span className={styles.buttons} data-bf-part="buttons">
            <button aria-label={decrementLabel} disabled={disabled || value <= min} onClick={() => changeBy(-step)} tabIndex={-1} type="button">−</button>
            <button aria-label={incrementLabel} disabled={disabled || value >= max} onClick={() => changeBy(step)} tabIndex={-1} type="button">+</button>
          </span>
        )}
      </span>
    </span>
  );
});
