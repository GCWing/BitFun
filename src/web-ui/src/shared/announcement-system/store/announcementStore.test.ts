// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { announcementService } from '../services/AnnouncementService';
import type { AnnouncementCard, ModalPresentation } from '../types';
import { useAnnouncementStore } from './announcementStore';

function card({
  id,
  cardType = 'feature',
  delayMs = 0,
  presentation,
}: {
  id: string;
  cardType?: AnnouncementCard['card_type'];
  delayMs?: number;
  presentation?: ModalPresentation;
}): AnnouncementCard {
  return {
    id,
    card_type: cardType,
    source: 'local',
    app_version: '1.0.0',
    priority: 0,
    trigger: {
      condition: { type: 'always' },
      delay_ms: delayMs,
      once_per_version: true,
    },
    toast: {
      icon: '',
      title: id,
      description: id,
      action_label: '',
      dismissible: true,
      auto_dismiss_ms: null,
    },
    modal: presentation
      ? {
        size: 'xl',
        presentation,
        closable: true,
        pages: [{ layout: 'text_only', title: id, body: id, media: null }],
        completion_action: 'dismiss',
      }
      : null,
    expires_at: null,
  };
}

describe('announcement presentation scheduling', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useAnnouncementStore.getState().resetForDebug();
    vi.spyOn(announcementService, 'markSeen').mockResolvedValue();
    vi.spyOn(announcementService, 'dismiss').mockResolvedValue();
    vi.spyOn(announcementService, 'neverShow').mockResolvedValue();
  });

  afterEach(() => {
    useAnnouncementStore.getState().resetForDebug();
    document.body.replaceChildren();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('opens an announcement modal directly without routing through a toast', () => {
    const releaseLetter = card({
      id: 'release_letter_1_0_0',
      cardType: 'announcement',
      presentation: 'release_letter',
    });

    useAnnouncementStore.getState().loadQueue([releaseLetter]);
    vi.advanceTimersByTime(0);

    const state = useAnnouncementStore.getState();
    expect(state.openModal?.id).toBe(releaseLetter.id);
    expect(state.modalVisible).toBe(true);
    expect(state.activeToast).toBeNull();
    expect(announcementService.markSeen).not.toHaveBeenCalled();
  });

  it('applies each queued card delay after the previous card has completed', () => {
    const first = card({ id: 'first', delayMs: 1000 });
    const second = card({ id: 'second', delayMs: 2000 });

    useAnnouncementStore.getState().loadQueue([first, second]);
    vi.advanceTimersByTime(999);
    expect(useAnnouncementStore.getState().activeToast).toBeNull();
    vi.advanceTimersByTime(1);
    expect(useAnnouncementStore.getState().activeToast?.id).toBe(first.id);

    useAnnouncementStore.getState().dismissToast(first);
    vi.advanceTimersByTime(400 + 1999);
    expect(useAnnouncementStore.getState().activeToast).toBeNull();
    vi.advanceTimersByTime(1);
    expect(useAnnouncementStore.getState().activeToast?.id).toBe(second.id);
  });

  it('defers a direct modal while another blocking dialog is active', () => {
    const blocker = document.createElement('div');
    blocker.setAttribute('role', 'alertdialog');
    blocker.setAttribute('aria-modal', 'true');
    document.body.append(blocker);
    const releaseLetter = card({
      id: 'release_letter_1_0_0',
      cardType: 'announcement',
      presentation: 'release_letter',
    });

    useAnnouncementStore.getState().loadQueue([releaseLetter]);
    vi.advanceTimersByTime(0);
    expect(useAnnouncementStore.getState().openModal).toBeNull();

    blocker.remove();
    vi.advanceTimersByTime(250);
    expect(useAnnouncementStore.getState().openModal?.id).toBe(releaseLetter.id);
  });

  it('keeps development previews entirely out of durable state', () => {
    const releaseLetter = card({
      id: 'release_letter_1_0_0',
      cardType: 'announcement',
      presentation: 'release_letter',
    });

    useAnnouncementStore.getState().forceShowCards([releaseLetter]);
    vi.advanceTimersByTime(0);
    useAnnouncementStore.getState().markModalPresented(releaseLetter);
    useAnnouncementStore.getState().closeModal();

    expect(announcementService.markSeen).not.toHaveBeenCalled();
    expect(announcementService.dismiss).not.toHaveBeenCalled();
    expect(announcementService.neverShow).not.toHaveBeenCalled();
  });
});
