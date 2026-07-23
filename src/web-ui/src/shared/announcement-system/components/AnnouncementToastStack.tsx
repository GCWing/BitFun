import React from 'react';
import { useAnnouncementStore } from '../store/announcementStore';
import AnnouncementToastItem from './AnnouncementToastItem';
import '../styles/AnnouncementToast.scss';

/**
 * Fixed bottom-left announcement stack.
 *
 * Shows the active toast on top. When more cards are queued, up to two ghost
 * layers peek out beneath it to hint that more notifications are waiting —
 * no text badge is shown.
 */
const AnnouncementToastStack: React.FC = () => {
  const { activeToast, toastVisible, queue } = useAnnouncementStore();

  if (!activeToast || !toastVisible) return null;

  const ghostCount = Math.min(queue.length, 2);

  return (
    <div className="announcement-toast-stack" aria-label="Announcements">
      <div className="announcement-toast-deck">
        {/* Ghost layers: rendered before active card = lower in DOM = behind */}
        {ghostCount >= 2 && (
          <div className="announcement-toast-ghost announcement-toast-ghost--2" aria-hidden />
        )}
        {ghostCount >= 1 && (
          <div className="announcement-toast-ghost announcement-toast-ghost--1" aria-hidden />
        )}
        <AnnouncementToastItem card={activeToast} />
      </div>
    </div>
  );
};

export default AnnouncementToastStack;
