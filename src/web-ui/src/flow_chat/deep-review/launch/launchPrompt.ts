interface SessionFilesLaunchPromptParams {
  extraContext?: string;
  reviewTeamPromptBlock: string;
}

interface SlashCommandLaunchPromptParams {
  extraContext: string;
  reviewTeamPromptBlock: string;
}

const REVIEW_PROMPT_CONTEXT_CHAR_LIMIT = 8_000;

function boundedPromptText(value: string, limit: number): string {
  if (value.length <= limit) {
    return value;
  }
  return `${value.slice(0, limit)}\n... Omitted ${value.length - limit} characters from the launch prompt.`;
}

function formatFocus(extraContext?: string): string {
  const focus = extraContext?.trim();
  return focus
    ? `User-provided focus:\n${boundedPromptText(focus, REVIEW_PROMPT_CONTEXT_CHAR_LIMIT)}`
    : 'User-provided focus:\nNone.';
}

export function formatSessionFilesLaunchPrompt({
  extraContext,
  reviewTeamPromptBlock,
}: SessionFilesLaunchPromptParams): string {
  return [
    'Run the prepared read-only Review plan below.',
    'The target and scopes are already resolved. Do not infer another target from filenames or focus text.',
    formatFocus(extraContext),
    reviewTeamPromptBlock,
  ].join('\n\n');
}

export function formatSlashCommandLaunchPrompt({
  extraContext,
  reviewTeamPromptBlock,
}: SlashCommandLaunchPromptParams): string {
  return [
    'Run the prepared read-only Review plan below.',
    'The slash-command target is already resolved. Do not reinterpret refs, paths, or scope from the focus text.',
    formatFocus(extraContext),
    reviewTeamPromptBlock,
  ].join('\n\n');
}
