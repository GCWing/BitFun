import React, { useEffect, useRef } from 'react';
import { X } from 'lucide-react';
import { useAnnouncementStore } from '../store/announcementStore';
import FeatureModalPage from './FeatureModalPage';
import { useAnnouncementI18n } from '../hooks/useAnnouncementI18n';
import '../styles/FeatureModal.scss';
import type { ModalConfig } from '../types';

/**
 * Centre-screen feature demo modal.
 *
 * - Multi-page with dot-indicator navigation.
 * - Each page can carry a media asset (Lottie, video, image, GIF).
 * - Backdrop click closes the modal when `closable` is true.
 * - Supports a "Don't show again" completion action.
 */
const FeatureModal: React.FC = () => {
  const { t } = useAnnouncementI18n();
  const {
    openModal,
    modalVisible,
    currentPage,
    setPage,
    closeModal,
  } = useAnnouncementStore();

  const [exiting, setExiting] = React.useState(false);
  const backdropRef = useRef<HTMLDivElement>(null);

  // When modalVisible becomes false trigger the exit animation.
  useEffect(() => {
    if (!modalVisible) setExiting(false);
  }, [modalVisible]);

  if (!openModal || !modalVisible) return null;

  const modal: ModalConfig = openModal.modal!;
  const pages = modal.pages ?? [];
  const isFirst = currentPage === 0;
  const isLast = currentPage === pages.length - 1;

  function triggerClose(neverShow = false) {
    setExiting(true);
    setTimeout(() => closeModal(neverShow), 280);
  }

  function handleBackdropClick(e: React.MouseEvent) {
    if (e.target === backdropRef.current && modal.closable) {
      triggerClose();
    }
  }

  const sizeClass = `feature-modal--${modal.size ?? 'lg'}`;

  return (
    <div
      ref={backdropRef}
      className={`feature-modal-backdrop${exiting ? ' feature-modal-backdrop--exiting' : ''}`}
      onClick={handleBackdropClick}
      role="dialog"
      aria-modal="true"
    >
      <div className={`feature-modal ${sizeClass}${exiting ? ' feature-modal--exiting' : ''}`}>
        {/* Close button */}
        {modal.closable && (
          <button
            type="button"
            className="feature-modal__close"
            onClick={() => triggerClose()}
            aria-label={t('announcements.common.close')}
          >
            <X size={14} strokeWidth={2} />
          </button>
        )}

        {/* Page viewport */}
        <div className="feature-modal__pages">
          {pages.map((page, i) => (
            <div
              key={i}
              style={{ display: i === currentPage ? 'block' : 'none' }}
            >
              <FeatureModalPage page={page} active={i === currentPage} />
            </div>
          ))}
        </div>

        {/* Footer navigation */}
        <div className="feature-modal__footer">
          {/* Dot indicators */}
          <div className="feature-modal__dots" aria-label="Page navigation">
            {pages.map((_, i) => (
              <button
                key={i}
                type="button"
                className={`feature-modal__dot${i === currentPage ? ' feature-modal__dot--active' : ''}`}
                onClick={() => setPage(i)}
                aria-label={`${t('announcements.common.page')} ${i + 1}`}
              />
            ))}
          </div>

          {/* Navigation buttons */}
          <div className="feature-modal__nav">
            {modal.completion_action === 'never_show_again' && isLast && (
              <button
                type="button"
                className="feature-modal__never"
                onClick={() => triggerClose(true)}
              >
                {t('announcements.common.never_show_again')}
              </button>
            )}
            {!isFirst && (
              <button
                type="button"
                className="feature-modal__nav-btn feature-modal__nav-btn--prev"
                onClick={() => setPage(currentPage - 1)}
              >
                {t('announcements.common.prev')}
              </button>
            )}
            {!isLast ? (
              <button
                type="button"
                className="feature-modal__nav-btn feature-modal__nav-btn--next"
                onClick={() => setPage(currentPage + 1)}
              >
                {t('announcements.common.next')}
              </button>
            ) : (
              <button
                type="button"
                className="feature-modal__nav-btn feature-modal__nav-btn--done"
                onClick={() => triggerClose()}
              >
                {t('announcements.common.done')}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default FeatureModal;
