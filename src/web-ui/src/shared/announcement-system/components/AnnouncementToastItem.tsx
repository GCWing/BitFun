import React, { useEffect, useRef, useState } from 'react';
import { X } from 'lucide-react';
import type { AnnouncementCard } from '../types';
import { useAnnouncementStore } from '../store/announcementStore';
import { useAnnouncementI18n } from '../hooks/useAnnouncementI18n';

interface Props {
  card: AnnouncementCard;
}

/**
 * Bottom-left toast: compact fixed-width card.
 * Layout (top → bottom): title row (+ close) → description → action buttons.
 */
const AnnouncementToastItem: React.FC<Props> = ({ card }) => {
  const { t } = useAnnouncementI18n();
  const { openModalFor, dismissToast } = useAnnouncementStore();
  const [exiting, setExiting] = useState(false);
  const autoDismissTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { toast, card_type, modal } = card;
  const hasModal = card_type !== 'tip' && modal !== null;
  const autoDismissMs = toast.auto_dismiss_ms;

  const resolve = (key: string) => (key.startsWith('announcements.') ? t(key) : key);

  function triggerExit(callback: () => void) {
    if (autoDismissTimer.current) clearTimeout(autoDismissTimer.current);
    setExiting(true);
    setTimeout(callback, 280);
  }

  function handleDismiss() {
    triggerExit(() => dismissToast(card));
  }

  function handleAction() {
    if (hasModal) {
      triggerExit(() => openModalFor(card));
    } else {
      handleDismiss();
    }
  }

  useEffect(() => {
    if (autoDismissMs) {
      autoDismissTimer.current = setTimeout(handleDismiss, autoDismissMs);
    }
    return () => {
      if (autoDismissTimer.current) clearTimeout(autoDismissTimer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [card.id]);

  const actionLabel =
    resolve(toast.action_label) ||
    (hasModal ? t('announcements.common.learn_more') : t('announcements.common.got_it'));

  return (
    <div
      className={`announcement-toast ${exiting ? 'announcement-toast--exiting' : 'announcement-toast--entering'}`}
      role="alert"
      aria-live="polite"
    >
      {/* Row 1: title + close (with optional countdown ring) */}
      <div className="announcement-toast__header">
        <div className="announcement-toast__title">{resolve(toast.title)}</div>
        {toast.dismissible && (
          <div className="announcement-toast__close-wrap">
            {autoDismissMs && (
              <svg
                className="announcement-toast__ring"
                viewBox="0 0 28 28"
                aria-hidden
              >
                <circle cx="14" cy="14" r="11.5"
                  className="announcement-toast__ring-track" />
                <circle cx="14" cy="14" r="11.5"
                  className="announcement-toast__ring-fill"
                  style={{ animationDuration: `${autoDismissMs}ms` }} />
              </svg>
            )}
            <button
              type="button"
              className="announcement-toast__close"
              onClick={handleDismiss}
              aria-label={t('announcements.common.close')}
            >
              <X strokeWidth={2} />
            </button>
          </div>
        )}
      </div>

      {/* Row 2: description */}
      <p className="announcement-toast__desc">{resolve(toast.description)}</p>

      {/* Row 3: action buttons */}
      <div className="announcement-toast__actions">
        <button
          type="button"
          className="announcement-toast__btn announcement-toast__btn--primary"
          onClick={handleAction}
        >
          {actionLabel}
        </button>
      </div>

    </div>
  );
};

export default AnnouncementToastItem;
