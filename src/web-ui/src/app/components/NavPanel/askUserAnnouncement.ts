export type AnnouncementTranslator = (
  key: string,
  params?: Record<string, unknown>,
) => string;

/** Compute the aria-live message for an AskUserQuestion state transition. */
export function computeAnnouncementMessage(
  prevTitles: Map<string, string>,
  currentTitles: Map<string, string>,
  t: AnnouncementTranslator,
): string {
  const added: string[] = [];
  const removed: string[] = [];
  for (const [id, title] of currentTitles) {
    if (!prevTitles.has(id)) added.push(title);
  }
  for (const [id, title] of prevTitles) {
    if (!currentTitles.has(id)) removed.push(title);
  }

  if (added.length > 0) {
    return added.length === 1
      ? t('nav.sessions.ariaNeedsInputWithName', { name: added[0] })
      : t('nav.sessions.ariaNeedsInputPlural', { count: currentTitles.size });
  }
  if (removed.length > 0) {
    if (currentTitles.size === 0) {
      return t('nav.sessions.ariaInputResolved');
    }
    return t('nav.sessions.ariaInputResolvedRemaining', {
      name: removed[0],
      count: currentTitles.size,
    });
  }
  return '';
}
