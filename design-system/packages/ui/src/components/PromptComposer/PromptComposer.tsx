import {
  forwardRef,
  type ReactNode,
  type TextareaHTMLAttributes,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./PromptComposer.module.css";

export interface PromptComposerProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  endControls?: ReactNode;
  startControls?: ReactNode;
  "data-bf-preview-state"?: "focus";
}

export const PromptComposer = forwardRef<HTMLTextAreaElement, PromptComposerProps>(
  function PromptComposer({
    className,
    disabled,
    endControls,
    startControls,
    "data-bf-preview-state": previewState,
    ...props
  }, ref) {
    return (
      <div
        className={classNames(styles.root, className)}
        data-bf-component="prompt-composer"
        data-bf-preview-state={previewState}
        data-disabled={disabled || undefined}
      >
        <textarea
          {...props}
          className={styles.input}
          disabled={disabled}
          ref={ref}
        />
        {(startControls || endControls) && (
          <div className={styles.controls} data-bf-part="controls">
            <div className={styles.controlGroup} data-bf-part="start-controls">{startControls}</div>
            <div className={styles.controlGroup} data-bf-part="end-controls">{endControls}</div>
          </div>
        )}
      </div>
    );
  },
);
