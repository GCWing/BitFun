import {
  cloneElement,
  forwardRef,
  useId,
  type HTMLAttributes,
  type ReactElement,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Field.module.css";

interface FieldControlProps {
  "aria-describedby"?: string;
  "aria-invalid"?: boolean | "true" | "false";
  id?: string;
  required?: boolean;
}

export type FieldControlWidth = "auto" | "fill";
export type FieldHorizontalGap = "md" | "lg";
export type FieldLabelWidth = "auto" | "sm" | "md" | "lg";

export interface FieldProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  children: ReactElement<FieldControlProps>;
  controlLeading?: ReactNode;
  controlTrailing?: ReactNode;
  controlWidth?: FieldControlWidth;
  description?: ReactNode;
  /** Validation message rendered below the control; also marks the control invalid. */
  error?: ReactNode;
  horizontalGap?: FieldHorizontalGap;
  label: ReactNode;
  labelAction?: ReactNode;
  labelWidth?: FieldLabelWidth;
  orientation?: "horizontal" | "vertical";
  required?: boolean;
}

export const Field = forwardRef<HTMLDivElement, FieldProps>(function Field({
  children,
  className,
  controlLeading,
  controlTrailing,
  controlWidth = "auto",
  description,
  error,
  horizontalGap = "md",
  label,
  labelAction,
  labelWidth = "auto",
  orientation = "vertical",
  required = false,
  ...props
}, ref) {
  const generatedId = useId();
  const controlId = children.props.id ?? `bf-field-${generatedId}`;
  const descriptionId = description === undefined || description === null
    ? undefined
    : `${controlId}-description`;
  const hasError = error !== undefined && error !== null && error !== false;
  const errorId = hasError ? `${controlId}-error` : undefined;
  const describedBy = [children.props["aria-describedby"], descriptionId, errorId]
    .filter((value): value is string => Boolean(value))
    .join(" ") || undefined;
  const isRequired = required || children.props.required === true;
  const control = cloneElement(children, {
    "aria-describedby": describedBy,
    "aria-invalid": hasError ? true : children.props["aria-invalid"],
    id: controlId,
    required: isRequired || undefined,
  });

  return (
    <div
      {...props}
      className={classNames(styles.root, className)}
      data-bf-component="field"
      data-control-width={controlWidth}
      data-horizontal-gap={horizontalGap}
      data-label-width={labelWidth}
      data-invalid={hasError ? "true" : undefined}
      data-orientation={orientation}
      data-required={isRequired ? "true" : "false"}
      ref={ref}
    >
      <span className={styles.content} data-bf-part="content">
        <span className={styles.labelRow} data-bf-part="label-row">
          <label className={styles.label} htmlFor={controlId}>
            <span>{label}</span>
            {isRequired && (
              <span aria-hidden="true" className={styles.required} data-bf-part="required">
                *
              </span>
            )}
          </label>
          {labelAction !== undefined && labelAction !== null && (
            <span className={styles.labelAction} data-bf-part="label-action">
              {labelAction}
            </span>
          )}
        </span>
        {descriptionId !== undefined && (
          <span className={styles.description} data-bf-part="description" id={descriptionId}>
            {description}
          </span>
        )}
      </span>
      <span className={styles.control} data-bf-part="control">
        {controlLeading !== undefined && controlLeading !== null && (
          <span className={styles.controlAdornment} data-bf-part="control-leading">
            {controlLeading}
          </span>
        )}
        {control}
        {controlTrailing !== undefined && controlTrailing !== null && (
          <span className={styles.controlAdornment} data-bf-part="control-trailing">
            {controlTrailing}
          </span>
        )}
      </span>
      {hasError && (
        <span className={styles.error} data-bf-part="error" id={errorId}>
          {error}
        </span>
      )}
    </div>
  );
});
