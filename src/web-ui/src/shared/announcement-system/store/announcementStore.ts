/**
 * Announcement system Zustand store.
 *
 * Manages the queue of pending cards, the currently visible toast, and the
 * centre-modal state.  All side-effects (Tauri calls) are delegated to
 * `AnnouncementService`; this store is pure state management.
 */
import { create } from 'zustand';
import type { AnnouncementCard } from '../types';
import { announcementService } from '../services/AnnouncementService';

export interface AnnouncementStoreState {
  /** Ordered list of cards waiting to be displayed. */
  queue: AnnouncementCard[];
  /** The card currently shown in the left-bottom toast area. */
  activeToast: AnnouncementCard | null;
  /** Whether the toast is visible (controls enter/exit animation). */
  toastVisible: boolean;
  /** The card whose modal is currently open. */
  openModal: AnnouncementCard | null;
  /** Whether the modal is visible (controls enter/exit animation). */
  modalVisible: boolean;
  /** Current page index inside the open modal. */
  currentPage: number;
  /** Whether the system has been initialised for this session. */
  initialised: boolean;
}

export interface AnnouncementStoreActions {
  /** Load the queue returned from the backend scheduler. */
  loadQueue(cards: AnnouncementCard[]): void;
  /** Show the next card from the queue as a toast. */
  showNextToast(): void;
  /** User clicked the toast's primary action – open the full modal. */
  openModalFor(card: AnnouncementCard): void;
  /** Navigate inside the modal. */
  setPage(page: number): void;
  /** Close the toast (x button or auto-dismiss). */
  dismissToast(card: AnnouncementCard): void;
  /** Close the modal and advance to the next card in the queue. */
  closeModal(neverShow?: boolean): void;
  /** Mark initialisation complete so the Provider does not re-run. */
  markInitialised(): void;
  /**
   * DEBUG ONLY — directly inject cards into the queue, bypassing backend
   * filters and the `initialised` guard.  Intended for dev-mode key trigger.
   */
  forceShowCards(cards: AnnouncementCard[]): void;
  /** Reset `initialised` so the Provider will re-fetch on next render. */
  resetForDebug(): void;
}

type AnnouncementStore = AnnouncementStoreState & AnnouncementStoreActions;

export const useAnnouncementStore = create<AnnouncementStore>((set, get) => ({
  queue: [],
  activeToast: null,
  toastVisible: false,
  openModal: null,
  modalVisible: false,
  currentPage: 0,
  initialised: false,

  loadQueue(cards) {
    set({ queue: cards });
    // Kick off the first toast if there are any cards.
    if (cards.length > 0) {
      get().showNextToast();
    }
  },

  showNextToast() {
    const { queue } = get();
    if (queue.length === 0) return;
    const [next, ...rest] = queue;
    set({ queue: rest, activeToast: next, toastVisible: true, currentPage: 0 });
  },

  openModalFor(card) {
    announcementService.markSeen(card.id);
    set({ toastVisible: false, openModal: card, modalVisible: true, currentPage: 0 });
  },

  setPage(page) {
    set({ currentPage: page });
  },

  dismissToast(card) {
    announcementService.dismiss(card.id);
    set({ toastVisible: false, activeToast: null });
    // Delay before showing next to allow exit animation to finish.
    setTimeout(() => get().showNextToast(), 400);
  },

  closeModal(neverShow = false) {
    const { openModal } = get();
    if (openModal) {
      if (neverShow) {
        announcementService.neverShow(openModal.id);
      } else {
        announcementService.dismiss(openModal.id);
      }
    }
    set({ modalVisible: false });
    // Wait for modal exit animation then advance to next card.
    setTimeout(() => {
      set({ openModal: null, currentPage: 0 });
      get().showNextToast();
    }, 350);
  },

  markInitialised() {
    set({ initialised: true });
  },

  forceShowCards(cards) {
    // Close anything currently open, then replace the queue.
    set({
      modalVisible: false,
      openModal: null,
      toastVisible: false,
      activeToast: null,
      currentPage: 0,
      queue: cards,
    });
    if (cards.length > 0) {
      setTimeout(() => get().showNextToast(), 100);
    }
  },

  resetForDebug() {
    set({ initialised: false });
  },
}));
