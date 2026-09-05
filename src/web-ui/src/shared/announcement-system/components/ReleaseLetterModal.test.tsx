// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { announcementService } from '../services/AnnouncementService';
import type { AnnouncementCard } from '../types';
import { useAnnouncementStore } from '../store/announcementStore';
import ReleaseLetterModal from './ReleaseLetterModal';
import { CONTENT_AT, INTRO_MS, LETTER_END, SIGNATURE_AT } from './releaseLetterMotion';

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
  let frames: Map<number, FrameRequestCallback>;
  let nextFrame: number;
  let motionQuery: EventTarget & { matches: boolean };

  function renderFrame(timestamp: number) {
    const callbacks = Array.from(frames.values());
    frames.clear();
    act(() => callbacks.forEach(callback => callback(timestamp)));
  }

  function openLetter() {
    useAnnouncementStore.getState().loadQueue([releaseLetter()]);
    vi.advanceTimersByTime(0);
    act(() => root.render(<ReleaseLetterModal />));
    renderFrame(0);
  }

  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    frames = new Map();
    nextFrame = 0;
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frames.set(++nextFrame, callback);
      return nextFrame;
    });
    vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id));
    motionQuery = Object.assign(new EventTarget(), { matches: false });
    vi.stubGlobal('matchMedia', () => motionQuery);
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
    vi.unstubAllGlobals();
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

  it('keeps the same drawing through construction, handoff, signature typing, and landing', () => {
    openLetter();
    const drawing = document.querySelector('.release-letter-drawing');
    expect(document.querySelector('.release-letter-scene')?.getAttribute('data-motion-state')).toBe('intro');
    expect(document.querySelectorAll('[data-typed="true"]')).toHaveLength(0);
    renderFrame(INTRO_MS + 100);
    expect(document.querySelector('.release-letter-scene')?.getAttribute('data-motion-state')).toBe('handoff');
    renderFrame(CONTENT_AT + 1200);
    expect(document.querySelector<HTMLElement>('.release-letter__title')?.style.opacity).toBe('1');
    expect(document.querySelectorAll('[data-typed="true"]')).toHaveLength(0);
    renderFrame(SIGNATURE_AT + 140);
    expect(document.querySelectorAll('[data-typed="true"]')).toHaveLength(3);
    expect(document.querySelector<HTMLButtonElement>('.release-letter__mascot-button')?.disabled).toBe(true);
    renderFrame(LETTER_END);
    expect(document.querySelector('.release-letter-drawing')).toBe(drawing);
    expect(document.querySelectorAll('[data-typed="true"]')).toHaveLength(15);
    expect(document.querySelector('.release-letter__mascot')?.getAttribute('data-settled')).toBe('true');
    expect(frames.size).toBe(0);
  });

  it('replays the connected mascot on hover and activation without replaying the letter or persisting again', () => {
    openLetter();
    renderFrame(LETTER_END);
    const button = document.querySelector<HTMLButtonElement>('.release-letter__mascot-button')!;
    const body = document.querySelector('[data-mascot="lift"]')!;
    const rest = body.getAttribute('transform');
    act(() => button.dispatchEvent(new MouseEvent('pointerover', { bubbles: true })));
    renderFrame(20000);
    renderFrame(20500);
    expect(body.getAttribute('transform')).not.toBe(rest);
    expect(document.querySelector('.release-letter-scene')?.getAttribute('data-motion-state')).toBe('settled');
    expect(document.querySelectorAll('[data-typed="true"]')).toHaveLength(15);
    expect(announcementService.markSeen).toHaveBeenCalledOnce();
    renderFrame(22000);
    expect(body.getAttribute('transform')).toBe(rest);
    expect(frames.size).toBe(0);
    act(() => button.click());
    expect(frames.size).toBe(1);
  });

  it('supports skipping and a full replay without rewriting seen state', () => {
    openLetter();
    act(() => document.querySelector<HTMLButtonElement>('.release-letter__skip')!.click());
    expect(document.querySelectorAll('[data-typed="true"]')).toHaveLength(15);
    expect(frames.size).toBe(0);
    act(() => document.querySelector<HTMLButtonElement>('.release-letter__version-mark')!.click());
    expect(document.querySelector('.release-letter-scene')?.getAttribute('data-motion-state')).toBe('intro');
    expect(document.querySelectorAll('[data-typed="true"]')).toHaveLength(0);
    expect(document.activeElement?.classList.contains('release-letter__skip')).toBe(true);
    expect(announcementService.markSeen).toHaveBeenCalledOnce();
  });

  it('shows the settled letter immediately with reduced motion and ignores jump replay', () => {
    motionQuery.matches = true;
    openLetter();
    expect(document.querySelector('.release-letter-scene')?.getAttribute('data-motion-state')).toBe('settled');
    expect(document.querySelectorAll('[data-typed="true"]')).toHaveLength(15);
    expect(document.querySelectorAll('[data-cursor="true"]')).toHaveLength(0);
    act(() => document.querySelector<HTMLButtonElement>('.release-letter__mascot-button')!.click());
    expect(frames.size).toBe(0);
  });

  it('pauses on visibility loss, settles on a motion preference change, and cancels frames when closed', () => {
    openLetter();
    renderFrame(1000);
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
    act(() => document.dispatchEvent(new Event('visibilitychange')));
    expect(frames.size).toBe(0);
    hidden.mockReturnValue(false);
    act(() => document.dispatchEvent(new Event('visibilitychange')));
    renderFrame(20000);
    expect(document.querySelector('.release-letter-scene')?.getAttribute('data-motion-state')).toBe('intro');
    motionQuery.matches = true;
    act(() => motionQuery.dispatchEvent(new Event('change')));
    expect(document.querySelector('.release-letter-scene')?.getAttribute('data-motion-state')).toBe('settled');
    expect(frames.size).toBe(0);
    motionQuery.matches = false;
    act(() => motionQuery.dispatchEvent(new Event('change')));
    act(() => document.querySelector<HTMLButtonElement>('.release-letter__version-mark')!.click());
    expect(frames.size).toBe(1);
    act(() => useAnnouncementStore.getState().closeModal());
    expect(frames.size).toBe(0);
  });
});
