/**
 * Announcement system Zustand store.
 *
 * Manages delayed card delivery, the currently visible toast, and modal state.
 * Persistence side-effects are delegated to `AnnouncementService`.
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
  /** IDs injected by a development preview. These never write durable state. */
  previewCardIds: ReadonlySet<string>;
  /** Whether the system has been initialised for this session. */
  initialised: boolean;
}

export interface AnnouncementStoreActions {
  /** Load the queue returned from the backend scheduler. */
  loadQueue(cards: AnnouncementCard[]): void;
  /** Schedule the next card using that card's own delay. */
  showNextCard(): void;
  /** User clicked the toast's primary action – open the full modal. */
  openModalFor(card: AnnouncementCard): void;
  /** A modal surface reports that it has actually mounted in the DOM. */
  markModalPresented(card: AnnouncementCard): void;
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

const BLOCKING_DIALOG_RETRY_MS = 250;
const TOAST_HANDOFF_MS = 400;
const MODAL_HANDOFF_MS = 220;

export function hasActiveBlockingDialog(root?: ParentNode): boolean {
  const searchRoot = root ?? (typeof document === 'undefined' ? null : document);
  if (!searchRoot) return false;

  const dialogs = searchRoot.querySelectorAll<HTMLElement>(
    '[role="dialog"][aria-modal="true"], [role="alertdialog"][aria-modal="true"]',
  );
  return Array.from(dialogs).some(dialog => (
    dialog.getAttribute('aria-hidden') !== 'true' && !dialog.hasAttribute('inert')
  ));
}

export const useAnnouncementStore = create<AnnouncementStore>((set, get) => {
  let presentationTimer: ReturnType<typeof setTimeout> | null = null;
  let handoffTimer: ReturnType<typeof setTimeout> | null = null;
  let scheduledCardId: string | null = null;
  const presentedCardIds = new Set<string>();

  const clearPresentationTimer = () => {
    if (presentationTimer !== null) clearTimeout(presentationTimer);
    presentationTimer = null;
    scheduledCardId = null;
  };

  const clearHandoffTimer = () => {
    if (handoffTimer !== null) clearTimeout(handoffTimer);
    handoffTimer = null;
  };

  const presentCard = (card: AnnouncementCard) => {
    presentationTimer = null;
    scheduledCardId = null;

    const state = get();
    if (state.activeToast || state.openModal || state.queue[0]?.id !== card.id) {
      return;
    }

    const opensDirectly = card.card_type === 'announcement' && card.modal !== null;
    if (opensDirectly && hasActiveBlockingDialog()) {
      scheduledCardId = card.id;
      presentationTimer = setTimeout(() => presentCard(card), BLOCKING_DIALOG_RETRY_MS);
      return;
    }

    const [, ...rest] = state.queue;
    if (opensDirectly) {
      set({
        queue: rest,
        activeToast: null,
        toastVisible: false,
        openModal: card,
        modalVisible: true,
        currentPage: 0,
      });
      return;
    }

    set({
      queue: rest,
      activeToast: card,
      toastVisible: true,
      currentPage: 0,
    });
  };

  return {
    queue: [],
    activeToast: null,
    toastVisible: false,
    openModal: null,
    modalVisible: false,
    currentPage: 0,
    previewCardIds: new Set<string>(),
    initialised: false,

  loadQueue(cards) {
    clearPresentationTimer();
    clearHandoffTimer();
    set({ queue: cards, previewCardIds: new Set<string>() });
    if (cards.length > 0) {
      get().showNextCard();
    }
  },

  showNextCard() {
    const state = get();
    if (
      state.queue.length === 0
      || state.activeToast
      || state.openModal
      || presentationTimer !== null
    ) return;

    const next = state.queue[0];
    const isPreview = state.previewCardIds.has(next.id);
    const delayMs = isPreview ? 0 : Math.max(0, next.trigger.delay_ms ?? 0);
    scheduledCardId = next.id;
    presentationTimer = setTimeout(() => {
      if (scheduledCardId === next.id) presentCard(next);
    }, delayMs);
  },

  openModalFor(card) {
    set({
      toastVisible: false,
      activeToast: null,
      openModal: card,
      modalVisible: true,
      currentPage: 0,
    });
  },

  markModalPresented(card) {
    if (get().previewCardIds.has(card.id) || presentedCardIds.has(card.id)) return;
    presentedCardIds.add(card.id);
    void announcementService.markSeen(card.id);
  },

  setPage(page) {
    set({ currentPage: page });
  },

  dismissToast(card) {
    if (!get().previewCardIds.has(card.id)) {
      void announcementService.dismiss(card.id);
    }
    set({ toastVisible: false, activeToast: null });
    clearHandoffTimer();
    handoffTimer = setTimeout(() => {
      handoffTimer = null;
      get().showNextCard();
    }, TOAST_HANDOFF_MS);
  },

  closeModal(neverShow = false) {
    const { openModal } = get();
    if (openModal && !get().previewCardIds.has(openModal.id)) {
      if (neverShow) {
        void announcementService.neverShow(openModal.id);
      } else {
        void announcementService.dismiss(openModal.id);
      }
    }
    set({ modalVisible: false });
    clearHandoffTimer();
    handoffTimer = setTimeout(() => {
      handoffTimer = null;
      set({ openModal: null, currentPage: 0 });
      get().showNextCard();
    }, MODAL_HANDOFF_MS);
  },

  markInitialised() {
    set({ initialised: true });
  },

  forceShowCards(cards) {
    clearPresentationTimer();
    clearHandoffTimer();
    set({
      modalVisible: false,
      openModal: null,
      toastVisible: false,
      activeToast: null,
      currentPage: 0,
      queue: cards,
      previewCardIds: new Set(cards.map(card => card.id)),
    });
    if (cards.length > 0) {
      get().showNextCard();
    }
  },

  resetForDebug() {
    clearPresentationTimer();
    clearHandoffTimer();
    presentedCardIds.clear();
    set({
      queue: [],
      activeToast: null,
      toastVisible: false,
      openModal: null,
      modalVisible: false,
      currentPage: 0,
      previewCardIds: new Set<string>(),
      initialised: false,
    });
  },
  };
});
