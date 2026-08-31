import {
  forwardRef,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./PageHeader.module.css";

type HeadingTag = "h1" | "h2" | "h3" | "h4" | "h5" | "h6";

export interface PageHeaderProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "title"> {
  action?: ReactNode;
  align?: "center" | "start";
  description?: ReactNode;
  leading?: ReactNode;
  level?: 1 | 2 | 3 | 4 | 5 | 6;
  required?: boolean;
  size?: "display" | "lg" | "md" | "sm";
  title: ReactNode;
}

export const PageHeader = forwardRef<HTMLDivElement, PageHeaderProps>(function PageHeader({
  action,
  align = "start",
  className,
  description,
  leading,
  level = 1,
  required = false,
  size = "md",
  title,
  ...props
}, ref) {
  const Heading = `h${level}` as HeadingTag;

  return (
    <div
      {...props}
      className={classNames(styles.root, className)}
      data-align={align}
      data-bf-component="page-header"
      data-level={level}
      data-required={required ? "true" : "false"}
      data-size={size}
      ref={ref}
    >
      {leading !== undefined && leading !== null && (
        <span className={styles.leading} data-bf-part="leading">
          {leading}
        </span>
      )}
      <span className={styles.content} data-bf-part="content">
        <Heading className={styles.heading} data-bf-part="heading">
          {title}
          {required && (
            <span aria-hidden="true" className={styles.required} data-bf-part="required">
              *
            </span>
          )}
        </Heading>
        {description !== undefined && description !== null && (
          <span className={styles.description} data-bf-part="description">
            {description}
          </span>
        )}
      </span>
      {action !== undefined && action !== null && (
        <span className={styles.action} data-bf-part="action">
          {action}
        </span>
      )}
    </div>
  );
});
