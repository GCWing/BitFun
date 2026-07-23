/**
 * Session title helpers
 */

/**
 * Create a temporary session title from the user's message.
 *
 * @param message User message
 * @param maxLength Maximum length (default 20 chars)
 * @returns Temporary title
 */
export function generateTempTitle(message: string, maxLength: number = 20): string {
  if (!message || message.trim().length === 0) {
    return 'New session';
  }

  const trimmed = message.trim();
  const normalized = trimmed.replace(/\s+/g, ' ');
  
  if (normalized.length <= maxLength) {
    return normalized;
  }
  
  let truncated = normalized.substring(0, maxLength);
  
  // Try to cut at punctuation or space for a more natural title.
  const breakPoints = [
    { char: '\u3002', offset: 0 },
    { char: '\uff01', offset: 0 },
    { char: '\uff1f', offset: 0 },
    { char: '\uff1b', offset: 0 },
    { char: '.', offset: 0 },
    { char: '!', offset: 0 },
    { char: '?', offset: 0 },
    { char: '\uff0c', offset: 0 },
    { char: ',', offset: 0 },
    { char: ' ', offset: 0 },
  ];
  
  let bestBreakPoint = -1;
  for (const { char } of breakPoints) {
    const index = truncated.lastIndexOf(char);
    if (index > maxLength / 2 && index > bestBreakPoint) {
      bestBreakPoint = index;
    }
  }
  
  if (bestBreakPoint > 0) {
    truncated = truncated.substring(0, bestBreakPoint + 1);
    const lastChar = truncated[truncated.length - 1];
    if ('\u3002\uff01\uff1f\uff1b.!?'.includes(lastChar)) {
      return truncated;
    }
  }
  
  return truncated + '...';
}

/**
 * Check whether a title is valid.
 */
export function isValidTitle(title: string): boolean {
  return Boolean(title && title.trim().length > 0 && title !== 'New session');
}

