import { forwardRef, type HTMLAttributes } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MediaThumbnail.module.css";

export type MediaThumbnailPresentation = "contain" | "placeholder" | "stacked";

export interface MediaThumbnailProps extends HTMLAttributes<HTMLDivElement> {
  alt: string;
  presentation?: MediaThumbnailPresentation;
  src: string;
}

export const MediaThumbnail = forwardRef<HTMLDivElement, MediaThumbnailProps>(
  function MediaThumbnail({ alt, className, presentation = "contain", src, ...props }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.root, className)}
        data-bf-component="media-thumbnail"
        data-bf-presentation={presentation}
        ref={ref}
      >
        {presentation === "stacked" && (
          <span aria-hidden="true" className={styles.backdrop} data-bf-part="backdrop" />
        )}
        <img alt={alt} className={styles.image} data-bf-part="image" src={src} />
      </div>
    );
  },
);
