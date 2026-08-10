import React from 'react';
import { Modal } from '@/component-library';
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
  actions?: React.ReactNode;
  children?: React.ReactNode;
  testId?: string;
  titleTestId?: string;
  descriptionTestId?: string;
  closeButtonTestId?: string;
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
  actions,
  children,
  testId,
  titleTestId,
  descriptionTestId,
  closeButtonTestId,
}) => (
  <Modal
    isOpen={isOpen}
    onClose={onClose}
    size="medium"
    title={title}
    contentInset
    testId={testId}
    titleTestId={titleTestId}
    closeButtonTestId={closeButtonTestId}
  >
    <div
      className="gallery-detail-modal"
      data-bf-component="gallery-detail-modal"
      data-bf-part="root"
    >
      <div className="gallery-detail-modal__hero" data-bf-component="gallery-detail-modal" data-bf-part="hero">
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
        <div className="gallery-detail-modal__summary" data-bf-component="gallery-detail-modal" data-bf-part="summary">
          {badges ? <div className="gallery-detail-modal__badges" data-bf-component="gallery-detail-modal" data-bf-part="badges">{badges}</div> : null}
          {description?.trim() ? (
            <p className="gallery-detail-modal__description" data-bf-component="gallery-detail-modal" data-bf-part="description" data-testid={descriptionTestId}>{description.trim()}</p>
          ) : null}
          {meta ? <div className="gallery-detail-modal__meta" data-bf-component="gallery-detail-modal" data-bf-part="meta">{meta}</div> : null}
        </div>
      </div>

      {children ? <div className="gallery-detail-modal__content" data-bf-component="gallery-detail-modal" data-bf-part="content">{children}</div> : null}

      {actions ? <div className="gallery-detail-modal__actions" data-bf-component="gallery-detail-modal" data-bf-part="actions">{actions}</div> : null}
    </div>
  </Modal>
);

export default GalleryDetailModal;
export type { GalleryDetailModalProps };
