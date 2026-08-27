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
  id?: string;
  required?: boolean;
}

export interface FieldProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  children: ReactElement<FieldControlProps>;
  controlClassName?: string;
  controlLeading?: ReactNode;
  controlTrailing?: ReactNode;
  description?: ReactNode;
  label: ReactNode;
  labelAction?: ReactNode;
  labelClassName?: string;
  orientation?: "horizontal" | "vertical";
  required?: boolean;
}

export const Field = forwardRef<HTMLDivElement, FieldProps>(function Field({
  children,
  className,
  controlClassName,
  controlLeading,
  controlTrailing,
  description,
  label,
  labelAction,
  labelClassName,
  orientation = "vertical",
  required = false,
  ...props
}, ref) {
  const generatedId = useId();
  const controlId = children.props.id ?? `bf-field-${generatedId}`;
  const descriptionId = description === undefined || description === null
    ? undefined
    : `${controlId}-description`;
  const describedBy = [children.props["aria-describedby"], descriptionId]
    .filter((value): value is string => Boolean(value))
    .join(" ") || undefined;
  const isRequired = required || children.props.required === true;
  const control = cloneElement(children, {
    "aria-describedby": describedBy,
    id: controlId,
    required: isRequired || undefined,
  });

  return (
    <div
      {...props}
      className={classNames(styles.root, className)}
      data-bf-component="field"
      data-orientation={orientation}
      data-required={isRequired ? "true" : "false"}
      ref={ref}
    >
      <span className={classNames(styles.content, labelClassName)} data-bf-part="content">
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
      <span className={classNames(styles.control, controlClassName)} data-bf-part="control">
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
    </div>
  );
});
