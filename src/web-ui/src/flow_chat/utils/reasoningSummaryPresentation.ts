function markdownToSingleLine(markdown: string): string {
  return markdown
    .replace(/```[\s\S]*?```/g, match => match.replace(/```[^\n]*\n?|```/g, ' '))
    .replace(/`([^`]*)`/g, '$1')
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/^\s{0,3}#{1,6}\s+/gm, '')
    .replace(/^\s{0,3}>\s?/gm, '')
    .replace(/^\s*[-*+]\s+/gm, '')
    .replace(/^\s*\d+\.\s+/gm, '')
    .replace(/[*_~]{1,3}/g, '')
    .replace(/<[^>]+>/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

/**
 * Return the latest OpenAI reasoning-summary part for the collapsed card.
 * The Responses adapter restores `summary_index` boundaries as blank lines;
 * the expanded card keeps the original Markdown while this preview is plain,
 * single-line text.
 */
export function latestReasoningSummaryPreview(content: string): string {
  const latestPart = content
    .split(/\n\s*\n/)
    .map(part => part.trim())
    .filter(Boolean)
    .at(-1);

  return latestPart ? markdownToSingleLine(latestPart) : '';
}
