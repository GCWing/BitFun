import React, { useEffect, useCallback } from 'react';
import { useAnnouncementStore } from '../store/announcementStore';
import { announcementService } from '../services/AnnouncementService';
import AnnouncementToastStack from './AnnouncementToastStack';
import FeatureModal from './FeatureModal';
import ReleaseLetterModal from './ReleaseLetterModal';
import { configAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import { scheduleAfterStartupPaint } from '@/shared/utils/startupTaskScheduling';

const log = createLogger('AnnouncementProvider');

/**
 * Card IDs that the debug trigger will cycle through in dev mode.
 *
 * Add any card ID here to include it in the Ctrl+Shift+Alt+D preview cycle.
 * Order determines display sequence.
 */
const DEBUG_CARD_IDS = ['release_letter_1_0_0'];

const ENV_PREVIEW_CARD_ID = import.meta.env.DEV
  ? import.meta.env.VITE_OPENBITFUN_ANNOUNCEMENT_PREVIEW_ID?.trim()
  : undefined;

/**
 * Application-level provider for the announcement system.
 *
 * Mount this once near the root of the app (after workspace loading is
 * complete, so the splash screen has already been shown).  It fetches pending
 * cards from the backend scheduler and passes them to the store, which then
 * drives the Toast and Modal rendering.
 *
 * The provider also renders the global UI surfaces:
 *   - <AnnouncementToastStack>  (bottom-left)
 *   - <FeatureModal>            (standard centre-screen presentation)
 *   - <ReleaseLetterModal>      (release-letter presentation)
 *
 * ─── Debug Mode ───────────────────────────────────────────────────────────────
 * In development (`import.meta.env.DEV`), press **Ctrl+Shift+Alt+D** to
 * inject all cards listed in `DEBUG_CARD_IDS` into the queue, bypassing
 * backend filter logic.  This lets you preview the full UI flow without
 * clearing persisted state.
 */
const AnnouncementProvider: React.FC<{ ready: boolean }> = ({ ready }) => {
  const { loadQueue, markInitialised, initialised, forceShowCards } = useAnnouncementStore();

  // ── Normal load path ─────────────────────────────────────────────────────
  useEffect(() => {
    if (!ready || initialised) return;
    let cancelled = false;

    const load = async () => {
      try {
        if (ENV_PREVIEW_CARD_ID) {
          const previewCard = await announcementService.triggerCard(ENV_PREVIEW_CARD_ID);
          if (cancelled) return;
          if (!previewCard) {
            log.warn('Announcement preview card was not found', { id: ENV_PREVIEW_CARD_ID });
            return;
          }
          log.debug('Force-showing announcement preview card', { id: previewCard.id });
          forceShowCards([previewCard]);
          return;
        }

        const cards = await announcementService.getPendingAnnouncements();
        if (cancelled) {
          return;
        }
        const tipsEnabled = await configAPI.getConfig('app.notifications.enable_startup_tips') !== false;
        if (cancelled) {
          return;
        }
        const visibleCards = tipsEnabled ? cards : cards.filter((card) => card.card_type !== 'tip');
        if (visibleCards.length > 0) {
          log.debug('Announcement cards loaded', { count: visibleCards.length });
          loadQueue(visibleCards);
        }
      } catch (e) {
        log.error('Failed to load announcement cards', e);
      } finally {
        if (!cancelled) {
          markInitialised();
        }
      }
    };

    const cancelStartupSchedule = scheduleAfterStartupPaint(() => {
      void load();
    }, {
      frameCount: 2,
      onError: error => {
        log.error('Failed to schedule announcement load after startup', error);
      },
    });

    return () => {
      cancelled = true;
      cancelStartupSchedule();
    };
  }, [forceShowCards, initialised, loadQueue, markInitialised, ready]);

  // ── Debug trigger ─────────────────────────────────────────────────────────
  const handleDebugTrigger = useCallback(async () => {
    log.debug('[DEBUG] Triggering announcement preview', { ids: DEBUG_CARD_IDS });
    try {
      const cards = await announcementService.debugTriggerCards(DEBUG_CARD_IDS);
      if (cards.length === 0) {
        log.warn('[DEBUG] No cards resolved for debug trigger. Check DEBUG_CARD_IDS.');
        return;
      }
      log.debug('[DEBUG] Force-showing cards', { count: cards.length, ids: cards.map((c) => c.id) });
      forceShowCards(cards);
    } catch (e) {
      log.error('[DEBUG] Failed to trigger debug cards', e);
    }
  }, [forceShowCards]);

  useEffect(() => {
    if (!import.meta.env.DEV || !ready) return;

    const onKeyDown = (e: KeyboardEvent) => {
      // Ctrl+Shift+Alt+D (all platforms)
      if (e.ctrlKey && e.shiftKey && e.altKey && e.key === 'D') {
        e.preventDefault();
        handleDebugTrigger();
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [handleDebugTrigger, ready]);

  if (!ready) return null;

  return (
    <>
      <AnnouncementToastStack />
      <FeatureModal />
      <ReleaseLetterModal />
    </>
  );
};

export default AnnouncementProvider;
