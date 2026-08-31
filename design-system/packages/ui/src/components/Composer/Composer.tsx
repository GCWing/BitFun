import {
  forwardRef,
  type FieldsetHTMLAttributes,
  type ForwardedRef,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Composer.module.css";

export interface ComposerProps
  extends Omit<FieldsetHTMLAttributes<HTMLFieldSetElement>, "children"> {
  children: ReactNode;
  contextBar?: ReactNode;
  disabled?: boolean;
  invalid?: boolean;
  toolbar?: ReactNode;
}

export interface ComposerBarProps extends HTMLAttributes<HTMLDivElement> {
  leading?: ReactNode;
  trailing?: ReactNode;
}

export type ComposerDividerProps = HTMLAttributes<HTMLSpanElement>;

export const Composer = forwardRef<HTMLFieldSetElement, ComposerProps>(
  function Composer({
    "aria-disabled": ariaDisabled,
    "aria-invalid": ariaInvalid,
    children,
    className,
    contextBar,
    disabled = false,
    invalid = false,
    toolbar,
    ...props
  }, ref) {
    const resolvedDisabled = disabled || ariaDisabled === true || ariaDisabled === "true";
    const resolvedInvalid = invalid || ariaInvalid === true || ariaInvalid === "true";

    return (
      <fieldset
        {...props}
        aria-disabled={resolvedDisabled || undefined}
        aria-invalid={resolvedInvalid || undefined}
        className={classNames(styles.root, className)}
        data-bf-component="composer"
        data-disabled={resolvedDisabled ? "true" : "false"}
        data-has-context={contextBar !== undefined && contextBar !== null ? "true" : "false"}
        data-invalid={resolvedInvalid ? "true" : "false"}
        disabled={resolvedDisabled}
        ref={ref}
      >
        {contextBar !== undefined && contextBar !== null && (
          <div className={styles.context} data-bf-part="context">
            {contextBar}
          </div>
        )}
        <div className={styles.surface} data-bf-part="surface">
          <div className={styles.editor} data-bf-part="editor">
            {children}
          </div>
          {toolbar !== undefined && toolbar !== null && (
            <div className={styles.toolbar} data-bf-part="toolbar">
              {toolbar}
            </div>
          )}
        </div>
      </fieldset>
    );
  },
);

function renderComposerBar({
  children,
  className,
  leading,
  trailing,
  ...props
}: ComposerBarProps, ref: ForwardedRef<HTMLDivElement>, part: string) {
  return (
    <div
      {...props}
      className={classNames(styles.bar, className)}
      data-bf-part={part}
      ref={ref}
    >
      {leading !== undefined && leading !== null && (
        <div className={styles.barLeading} data-bf-part="bar-leading">
          {leading}
        </div>
      )}
      {children !== undefined && children !== null && (
        <div className={styles.barContent} data-bf-part="bar-content">
          {children}
        </div>
      )}
      {trailing !== undefined && trailing !== null && (
        <div className={styles.barTrailing} data-bf-part="bar-trailing">
          {trailing}
        </div>
      )}
    </div>
  );
}

export const ComposerContextBar = forwardRef<HTMLDivElement, ComposerBarProps>(
  function ComposerContextBar(props, ref) {
    return renderComposerBar(props, ref, "context-bar");
  },
);

export const ComposerToolbar = forwardRef<HTMLDivElement, ComposerBarProps>(
  function ComposerToolbar(props, ref) {
    return renderComposerBar(props, ref, "toolbar-bar");
  },
);

export const ComposerDivider = forwardRef<HTMLSpanElement, ComposerDividerProps>(
  function ComposerDivider({ className, ...props }, ref) {
    return (
      <span
        {...props}
        aria-hidden="true"
        className={classNames(styles.divider, className)}
        data-bf-part="divider"
        ref={ref}
      />
    );
  },
);
