/**
 * Announcement system Tauri API client.
 *
 * Wraps all Tauri `invoke` calls for the announcement system so that the
 * rest of the frontend never touches `invoke` directly.
 */
import { invoke } from '@tauri-apps/api/core';
import type { AnnouncementCard } from '../types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('AnnouncementService');

export const announcementService = {
  /**
   * Fetch the ordered list of cards that should be displayed in this session.
   * This also triggers the scheduler (increments open-count, updates version).
   * Should be called once per application start.
   */
  async getPendingAnnouncements(): Promise<AnnouncementCard[]> {
    try {
      return await invoke<AnnouncementCard[]>('get_pending_announcements');
    } catch (e) {
      log.error('Failed to get pending announcements', e);
      return [];
    }
  },

  /** Mark a card as seen (modal was opened or action button was clicked). */
  async markSeen(id: string): Promise<void> {
    try {
      await invoke('mark_announcement_seen', { request: { id } });
    } catch (e) {
      log.error('Failed to mark announcement seen', { id, error: e });
    }
  },

  /** Dismiss a card for the current version cycle. */
  async dismiss(id: string): Promise<void> {
    try {
      await invoke('dismiss_announcement', { request: { id } });
    } catch (e) {
      log.error('Failed to dismiss announcement', { id, error: e });
    }
  },

  /** Permanently suppress a card. */
  async neverShow(id: string): Promise<void> {
    try {
      await invoke('never_show_announcement', { request: { id } });
    } catch (e) {
      log.error('Failed to suppress announcement', { id, error: e });
    }
  },

  /**
   * Manually trigger a specific card by ID.
   * Returns `null` if no card with that ID is registered.
   */
  async triggerCard(id: string): Promise<AnnouncementCard | null> {
    try {
      return await invoke<AnnouncementCard | null>('trigger_announcement', { request: { id } });
    } catch (e) {
      log.error('Failed to trigger announcement', { id, error: e });
      return null;
    }
  },

  /** Fetch all currently eligible tip cards (for a tips browser). */
  async getTips(): Promise<AnnouncementCard[]> {
    try {
      return await invoke<AnnouncementCard[]>('get_announcement_tips');
    } catch (e) {
      log.error('Failed to get announcement tips', e);
      return [];
    }
  },

  /**
   * DEBUG ONLY — trigger a set of known card IDs and return the resolved cards.
   *
   * `trigger_announcement` bypasses all scheduler filters (seen/dismissed/version),
   * making it ideal for in-dev testing of card UI without clearing persisted state.
   */
  async debugTriggerCards(ids: string[]): Promise<AnnouncementCard[]> {
    const results = await Promise.all(
      ids.map((id) => announcementService.triggerCard(id)),
    );
    return results.filter((c): c is AnnouncementCard => c !== null);
  },
};
