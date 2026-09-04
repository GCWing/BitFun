import React from 'react';
import {
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
  type DialogSize,
} from '@openbitfun/ui';
import './GalleryDetailModal.scss';

interface GalleryDetailModalProps {
  isOpen: boolean;
  onClose: () => void;
  icon?: React.ReactNode;
  iconGradient?: string;
  title: string;
  badges?: React.ReactNode;
  description?: string;
  meta?: React.ReactNode;
  heroActions?: React.ReactNode;
  actions?: React.ReactNode;
  children?: React.ReactNode;
  testId?: string;
  titleTestId?: string;
  descriptionTestId?: string;
  closeButtonTestId?: string;
  titlePlacement?: 'header' | 'hero';
  size?: DialogSize;
  stableHeight?: boolean;
}

const GalleryDetailModal: React.FC<GalleryDetailModalProps> = ({
  isOpen,
  onClose,
  icon,
  iconGradient,
  title,
  badges,
  description,
  meta,
  heroActions,
  actions,
  children,
  testId,
  titleTestId,
  descriptionTestId,
  closeButtonTestId,
  titlePlacement = 'header',
  size = 'md',
  stableHeight = false,
}) => {
  const heroTitleId = React.useId();
  const usesHeroTitle = titlePlacement === 'hero';
  const appearanceState = [
    usesHeroTitle ? 'heroTitle' : '',
    stableHeight ? 'stableHeight' : '',
  ].filter(Boolean).join(' ');
  const descriptionContent = description?.trim() ? (
    <p
      className="gallery-detail-modal__description"
      data-openbitfun-component="gallery-detail-modal"
      data-openbitfun-part="description"
      data-testid={descriptionTestId}
    >
      {description.trim()}
    </p>
  ) : null;
  const badgesContent = badges ? (
    <div
      className="gallery-detail-modal__badges"
      data-openbitfun-component="gallery-detail-modal"
      data-openbitfun-part="badges"
    >
      {badges}
    </div>
  ) : null;
  const metaContent = meta ? (
    <div
      className="gallery-detail-modal__meta"
      data-openbitfun-component="gallery-detail-modal"
      data-openbitfun-part="meta"
    >
      {meta}
    </div>
  ) : null;

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(nextOpen) => { if (!nextOpen) onClose(); }}
      size={size}
      aria-labelledby={usesHeroTitle ? heroTitleId : undefined}
      className={stableHeight ? 'gallery-detail-modal__surface--stable-height' : undefined}
      data-testid={testId}
    >
      <DialogHeader className={usesHeroTitle ? 'gallery-detail-modal__dialog-header--hero-title' : undefined}>
        {!usesHeroTitle ? (
          <DialogHeading>
            <DialogTitle data-testid={titleTestId}>{title}</DialogTitle>
          </DialogHeading>
        ) : null}
        <DialogClose data-testid={closeButtonTestId} />
      </DialogHeader>
      <DialogBody
        className={stableHeight ? 'gallery-detail-modal__dialog-body--stable-height' : undefined}
      >
      <div
        className={[
          'gallery-detail-modal',
          usesHeroTitle ? 'gallery-detail-modal--hero-title' : '',
          stableHeight ? 'gallery-detail-modal--stable-height' : '',
        ].filter(Boolean).join(' ')}
        data-openbitfun-component="gallery-detail-modal"
        data-openbitfun-part="root"
        data-openbitfun-state={appearanceState || undefined}
      >
        <div
          className="gallery-detail-modal__hero"
          data-openbitfun-component="gallery-detail-modal"
          data-openbitfun-part="hero"
        >
          {icon ? (
            <div
              className="gallery-detail-modal__icon"
              data-openbitfun-component="gallery-detail-modal"
              data-openbitfun-part="icon"
              style={iconGradient ? ({ '--gallery-detail-gradient': iconGradient } as React.CSSProperties) : undefined}
            >
              {icon}
            </div>
          ) : null}
          <div
            className="gallery-detail-modal__summary"
            data-openbitfun-component="gallery-detail-modal"
            data-openbitfun-part="summary"
          >
            {usesHeroTitle ? (
              <>
                <h2
                  id={heroTitleId}
                  className="gallery-detail-modal__title"
                  data-openbitfun-component="gallery-detail-modal"
                  data-openbitfun-part="title"
                  data-testid={titleTestId}
                >
                  {title}
                </h2>
                {descriptionContent}
                {badgesContent || metaContent ? (
                  <div className="gallery-detail-modal__details">
                    {badgesContent}
                    {metaContent}
                  </div>
                ) : null}
              </>
            ) : (
              <>
                {badgesContent}
                {descriptionContent}
                {metaContent}
              </>
            )}
          </div>
          {heroActions ? (
            <div
              className="gallery-detail-modal__hero-actions"
              data-openbitfun-component="gallery-detail-modal"
              data-openbitfun-part="heroActions"
            >
              {heroActions}
            </div>
          ) : null}
        </div>

        {children ? (
          <div
            className="gallery-detail-modal__content"
            data-openbitfun-component="gallery-detail-modal"
            data-openbitfun-part="content"
          >
            {children}
          </div>
        ) : null}

        {actions ? (
          <div
            className="gallery-detail-modal__actions"
            data-openbitfun-component="gallery-detail-modal"
            data-openbitfun-part="actions"
          >
            {actions}
          </div>
        ) : null}
      </div>
      </DialogBody>
    </Dialog>
  );
};

export default GalleryDetailModal;
export type { GalleryDetailModalProps };
