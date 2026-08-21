import React from 'react';
import { Modal, type ModalProps } from '@/component-library';
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
  size?: ModalProps['size'];
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
  size = 'medium',
  stableHeight = false,
}) => {
  const heroTitleId = React.useId();
  const usesHeroTitle = titlePlacement === 'hero';
  const overlayClassName = [
    usesHeroTitle ? 'gallery-detail-modal__overlay--hero-title' : '',
    stableHeight ? 'gallery-detail-modal__overlay--stable-height' : '',
  ].filter(Boolean).join(' ');
  const appearanceState = [
    usesHeroTitle ? 'heroTitle' : '',
    stableHeight ? 'stableHeight' : '',
  ].filter(Boolean).join(' ');
  const descriptionContent = description?.trim() ? (
    <p
      className="gallery-detail-modal__description"
      data-bf-component="gallery-detail-modal"
      data-bf-part="description"
      data-testid={descriptionTestId}
    >
      {description.trim()}
    </p>
  ) : null;
  const badgesContent = badges ? (
    <div
      className="gallery-detail-modal__badges"
      data-bf-component="gallery-detail-modal"
      data-bf-part="badges"
    >
      {badges}
    </div>
  ) : null;
  const metaContent = meta ? (
    <div
      className="gallery-detail-modal__meta"
      data-bf-component="gallery-detail-modal"
      data-bf-part="meta"
    >
      {meta}
    </div>
  ) : null;

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      size={size}
      title={usesHeroTitle ? undefined : title}
      contentInset
      overlayClassName={overlayClassName || undefined}
      testId={testId}
      titleTestId={usesHeroTitle ? undefined : titleTestId}
      closeButtonTestId={closeButtonTestId}
      ariaLabelledBy={usesHeroTitle ? heroTitleId : undefined}
    >
      <div
        className={[
          'gallery-detail-modal',
          usesHeroTitle ? 'gallery-detail-modal--hero-title' : '',
          stableHeight ? 'gallery-detail-modal--stable-height' : '',
        ].filter(Boolean).join(' ')}
        data-bf-component="gallery-detail-modal"
        data-bf-part="root"
        data-bf-state={appearanceState || undefined}
      >
        <div
          className="gallery-detail-modal__hero"
          data-bf-component="gallery-detail-modal"
          data-bf-part="hero"
        >
          {icon ? (
            <div
              className="gallery-detail-modal__icon"
              data-bf-component="gallery-detail-modal"
              data-bf-part="icon"
              style={iconGradient ? ({ '--gallery-detail-gradient': iconGradient } as React.CSSProperties) : undefined}
            >
              {icon}
            </div>
          ) : null}
          <div
            className="gallery-detail-modal__summary"
            data-bf-component="gallery-detail-modal"
            data-bf-part="summary"
          >
            {usesHeroTitle ? (
              <>
                <h2
                  id={heroTitleId}
                  className="gallery-detail-modal__title"
                  data-bf-component="gallery-detail-modal"
                  data-bf-part="title"
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
              data-bf-component="gallery-detail-modal"
              data-bf-part="heroActions"
            >
              {heroActions}
            </div>
          ) : null}
        </div>

        {children ? (
          <div
            className="gallery-detail-modal__content"
            data-bf-component="gallery-detail-modal"
            data-bf-part="content"
          >
            {children}
          </div>
        ) : null}

        {actions ? (
          <div
            className="gallery-detail-modal__actions"
            data-bf-component="gallery-detail-modal"
            data-bf-part="actions"
          >
            {actions}
          </div>
        ) : null}
      </div>
    </Modal>
  );
};

export default GalleryDetailModal;
export type { GalleryDetailModalProps };
