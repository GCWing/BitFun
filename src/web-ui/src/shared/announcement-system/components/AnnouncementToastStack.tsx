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
    <div className="announcement-toast-stack" aria-label="Announcements" data-bf-component="announcement" data-bf-part="stack">
      <div className="announcement-toast-deck" data-bf-component="announcement" data-bf-part="deck">
        {/* Ghost layers: rendered before active card = lower in DOM = behind */}
        {ghostCount >= 2 && (
          <div className="announcement-toast-ghost announcement-toast-ghost--2" aria-hidden data-bf-component="announcement" data-bf-part="ghost" data-bf-depth="2" />
        )}
        {ghostCount >= 1 && (
          <div className="announcement-toast-ghost announcement-toast-ghost--1" aria-hidden data-bf-component="announcement" data-bf-part="ghost" data-bf-depth="1" />
        )}
        <AnnouncementToastItem card={activeToast} />
      </div>
    </div>
  );
};

export default AnnouncementToastStack;
