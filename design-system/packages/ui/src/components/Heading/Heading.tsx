import {
  forwardRef,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Heading.module.css";

export type HeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;
export type HeadingVariant = "hero" | "page" | "section" | "subsection";

export interface HeadingProps extends Omit<HTMLAttributes<HTMLDivElement>, "title"> {
  action?: ReactNode;
  description?: ReactNode;
  level?: HeadingLevel;
  title: ReactNode;
  variant?: HeadingVariant;
}

export const Heading = forwardRef<HTMLDivElement, HeadingProps>(function Heading({
  action,
  className,
  description,
  level = 2,
  title,
  variant = "section",
  ...props
}, ref) {
  const Title = `h${level}` as const;
  return (
    <div
      {...props}
      className={classNames(styles.root, className)}
      data-bf-component="heading"
      data-bf-part="root"
      data-variant={variant}
      ref={ref}
    >
      <div className={styles.copy} data-bf-part="copy">
        <Title className={styles.title} data-bf-part="title">{title}</Title>
        {description && (
          <div className={styles.description} data-bf-part="description">
            {description}
          </div>
        )}
      </div>
      {action && <div className={styles.action} data-bf-part="action">{action}</div>}
    </div>
  );
});
