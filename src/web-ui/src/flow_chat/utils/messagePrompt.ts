import type { ContextItem } from '@/shared/types/context';
import { formatContextForPrompt } from '@/shared/utils/contextPrompt';

export function stripInlineImageTags(text: string): string {
  return text
    .replace(/#img:[^\s\n]+\s?/g, '')
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

export function buildPromptMessage(message: string, contexts: ContextItem[]): string {
  const aiTrimmedMessage = stripInlineImageTags(message.trim());
  if (contexts.length === 0) {
    return aiTrimmedMessage;
  }

  const fullContextSection = contexts
    .map(formatContextForPrompt)
    .filter(Boolean)
    .join('\n');

  return `${fullContextSection}\n\n${aiTrimmedMessage}`;
}
