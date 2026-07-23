/**
 * Convenience hook to access announcement store state and actions.
 */
import { useAnnouncementStore } from '../store/announcementStore';

export function useAnnouncement() {
  return useAnnouncementStore();
}
