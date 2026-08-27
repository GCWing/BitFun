import {
  createElement,
  forwardRef,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./FieldGroup.module.css";

export type FieldGroupAppearance = "plain" | "subtle";
export type FieldRowAlignment = "center" | "start";
export type FieldRowPadding = "none" | "md";
export type FormSectionHeading = "h2" | "h3" | "h4";

export interface FormSectionProps
  extends Omit<HTMLAttributes<HTMLElement>, "title"> {
  actions?: ReactNode;
  description?: ReactNode;
  headingAs?: FormSectionHeading;
  leading?: ReactNode;
  title?: ReactNode;
}

export interface FieldGroupProps extends HTMLAttributes<HTMLDivElement> {
  appearance?: FieldGroupAppearance;
  dividers?: boolean;
}

export interface FieldRowProps extends HTMLAttributes<HTMLDivElement> {
  align?: FieldRowAlignment;
  padding?: FieldRowPadding;
}

export const FormSection = forwardRef<HTMLElement, FormSectionProps>(
  function FormSection({
    actions,
    children,
    className,
    description,
    headingAs = "h2",
    leading,
    title,
    ...props
  }, ref) {
    return (
      <section
        {...props}
        className={classNames(styles.section, className)}
        data-bf-component="form-section"
        ref={ref}
      >
        {(title !== undefined && title !== null)
          || (description !== undefined && description !== null)
          || (leading !== undefined && leading !== null)
          || (actions !== undefined && actions !== null) ? (
            <div className={styles.header} data-bf-part="header">
              {(title !== undefined && title !== null)
                || (description !== undefined && description !== null)
                || (leading !== undefined && leading !== null) ? (
                  <div className={styles.headingRegion} data-bf-part="heading-region">
                    {leading !== undefined && leading !== null && (
                      <div className={styles.leading} data-bf-part="leading">{leading}</div>
                    )}
                    {(title !== undefined && title !== null)
                      || (description !== undefined && description !== null) ? (
                        <div className={styles.headingContent} data-bf-part="heading-content">
                          {title !== undefined && title !== null && createElement(
                            headingAs,
                            { className: styles.title, "data-bf-part": "title" },
                            title,
                          )}
                          {description !== undefined && description !== null && (
                            <div className={styles.description} data-bf-part="description">
                              {description}
                            </div>
                          )}
                        </div>
                      ) : null}
                  </div>
                ) : null}
              {actions !== undefined && actions !== null && (
                <div className={styles.actions} data-bf-part="actions">{actions}</div>
              )}
            </div>
          ) : null}
        {children}
      </section>
    );
  },
);

export const FieldGroup = forwardRef<HTMLDivElement, FieldGroupProps>(
  function FieldGroup({
    appearance = "subtle",
    children,
    className,
    dividers = true,
    ...props
  }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.group, className)}
        data-appearance={appearance}
        data-bf-component="field-group"
        data-dividers={dividers ? "true" : "false"}
        ref={ref}
      >
        {children}
      </div>
    );
  },
);

export const FieldRow = forwardRef<HTMLDivElement, FieldRowProps>(
  function FieldRow({
    align = "center",
    children,
    className,
    padding = "md",
    ...props
  }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.row, className)}
        data-align={align}
        data-bf-part="row"
        data-padding={padding}
        ref={ref}
      >
        {children}
      </div>
    );
  },
);
