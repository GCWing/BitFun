// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { announcementService } from '../services/AnnouncementService';
import type { AnnouncementCard } from '../types';
import { useAnnouncementStore } from '../store/announcementStore';
import ReleaseLetterModal from './ReleaseLetterModal';

function releaseLetter(): AnnouncementCard {
  return {
    id: 'release_letter_1_0_0',
    card_type: 'announcement',
    source: 'local',
    app_version: '1.0.0',
    priority: 1000,
    trigger: {
      condition: { type: 'always' },
      delay_ms: 0,
      once_per_version: true,
    },
    toast: {
      icon: '',
      title: 'OpenBitFun 1.0.0',
      description: 'Release letter',
      action_label: '',
      dismissible: true,
      auto_dismiss_ms: null,
    },
    modal: {
      size: 'xl',
      presentation: 'release_letter',
      closable: true,
      pages: [{
        layout: 'text_only',
        title: 'A letter to you',
        body: 'Literal <img src=x onerror=alert(1)> text.\n\n**Create with it.**',
        media: null,
      }],
      completion_action: 'dismiss',
    },
    expires_at: null,
  };
}

describe('ReleaseLetterModal', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
    useAnnouncementStore.getState().resetForDebug();
    vi.spyOn(announcementService, 'markSeen').mockResolvedValue();
    vi.spyOn(announcementService, 'dismiss').mockResolvedValue();
  });

  afterEach(() => {
    act(() => root.unmount());
    useAnnouncementStore.getState().resetForDebug();
    host.remove();
    document.body.replaceChildren();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('acknowledges the card only after the real dialog surface mounts', () => {
    const card = releaseLetter();
    useAnnouncementStore.getState().loadQueue([card]);
    vi.advanceTimersByTime(0);
    expect(announcementService.markSeen).not.toHaveBeenCalled();

    act(() => root.render(<ReleaseLetterModal />));

    expect(document.querySelector('[role="dialog"]')).not.toBeNull();
    expect(announcementService.markSeen).toHaveBeenCalledOnce();
    expect(announcementService.markSeen).toHaveBeenCalledWith(card.id);
  });

  it('renders authored copy as escaped DOM text with semantic emphasis', () => {
    useAnnouncementStore.getState().loadQueue([releaseLetter()]);
    vi.advanceTimersByTime(0);
    act(() => root.render(<ReleaseLetterModal />));

    const copy = document.querySelector('.release-letter__paragraphs');
    expect(copy?.textContent).toContain('<img src=x onerror=alert(1)>');
    expect(copy?.querySelector('img')).toBeNull();
    expect(copy?.querySelector('strong')?.textContent).toBe('Create with it.');
  });
});
