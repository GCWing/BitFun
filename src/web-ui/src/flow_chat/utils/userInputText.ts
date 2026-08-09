/**
 * User-input text normalization shared by runtime event handling, session
 * hydration, and transcript export.
 */

/**
 * Strip agent-internal XML wrapper tags from persisted / forwarded user inputs.
 * Handles both normal and forwarded-agent envelopes.
 */
export function cleanRemoteUserInput(raw: string): string {
  const s = raw.trim();
  const userQueryMatch = s.match(/<user_query>\s*([\s\S]*?)\s*<\/user_query>/);
  if (userQueryMatch) {
    return userQueryMatch[1].trim();
  }

  return s
    .replace(/<system(?:_|-)reminder>[\s\S]*?<\/system(?:_|-)reminder>/g, '')
    .trim();
}

/**
 * Display text for a persisted user message: the original authored text when
 * the turn recorded one, otherwise the stored content with wrappers removed.
 */
export function resolvePersistedUserMessageText(
  content: string,
  metadata?: Record<string, any>
): string {
  const originalText = metadata?.original_text;
  if (typeof originalText === 'string' && originalText.trim()) {
    return originalText;
  }
  return cleanRemoteUserInput(content || '');
}
