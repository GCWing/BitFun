/**
 * Collect assistant reply text from a dialog turn.
 * Only `type === 'text'` items are kept; thinking, tool, user-steering,
 * and image-analysis items are excluded so copying a reply never leaks
 * chain-of-thought or tool-call content.
 */
export function collectAssistantText(
  dialogTurn: { modelRounds?: { items?: any[] }[] },
  mode: 'all' | 'final' = 'all',
): string {
  const textParts: string[] = [];
  for (const modelRound of dialogTurn.modelRounds ?? []) {
    for (const item of modelRound.items ?? []) {
      if (item.type === 'text' && typeof item.content === 'string' && item.content.trim()) {
        textParts.push(item.content.trim());
      }
    }
  }
  if (textParts.length === 0) return '';
  return mode === 'final' ? textParts[textParts.length - 1] : textParts.join('\n\n');
}
