interface SessionFilesLaunchPromptParams {
  filePaths: string[];
  extraContext?: string;
  reviewTeamPromptBlock: string;
}

interface PullRequestLaunchPromptParams {
  filePaths: string[];
  extraContext?: string;
  diffContext?: string;
  reviewTeamPromptBlock: string;
}

interface SlashCommandLaunchPromptParams {
  commandText: string;
  extraContext: string;
  reviewTeamPromptBlock: string;
}

export function formatFileList(filePaths: string[]): string {
  return filePaths.map(filePath => `- ${filePath}`).join('\n');
}

export function formatSessionFilesLaunchPrompt({
  filePaths,
  extraContext,
  reviewTeamPromptBlock,
}: SessionFilesLaunchPromptParams): string {
  const contextBlock = extraContext?.trim()
    ? `User-provided focus:\n${extraContext.trim()}`
    : 'User-provided focus:\nNone.';

  return [
    'Run a deep code review using the parallel Code Review Team.',
    'Review scope: ONLY inspect the following files modified in this session.',
    formatFileList(filePaths),
    contextBlock,
    reviewTeamPromptBlock,
    'Keep the scope tight to the listed files unless a directly-related dependency must be read to confirm a finding.',
  ].join('\n\n');
}

export function formatPullRequestLaunchPrompt({
  filePaths,
  extraContext,
  diffContext,
  reviewTeamPromptBlock,
}: PullRequestLaunchPromptParams): string {
  const contextBlock = extraContext?.trim()
    ? `Pull request context:\n${extraContext.trim()}`
    : 'Pull request context:\nNone.';
  const diffBlock = diffContext?.trim()
    ? `Pull request provider diff:\n${diffContext.trim()}`
    : 'Pull request provider diff:\nNo provider diff was included. Confirm findings against the listed files and PR metadata.';

  return [
    'Run a deep code review using the parallel Code Review Team.',
    'Review scope: ONLY inspect the following files changed by this pull request.',
    formatFileList(filePaths),
    contextBlock,
    diffBlock,
    reviewTeamPromptBlock,
    'Treat the provider diff as the source of truth for what changed in the PR. Read repository files only to understand surrounding context or verify findings.',
  ].join('\n\n');
}

export function formatSlashCommandLaunchPrompt({
  commandText,
  extraContext,
  reviewTeamPromptBlock,
}: SlashCommandLaunchPromptParams): string {
  const contextBlock = extraContext
    ? `User-provided focus or target:\n${extraContext}`
    : 'User-provided focus or target:\nNone. If no explicit target is given, review the current workspace changes relative to HEAD.';

  return [
    'Run a deep code review using the parallel Code Review Team.',
    'Interpret the user command below to determine the review target.',
    'If the user mentions a commit, ref, branch, or explicit file set, review that target.',
    'Otherwise, review the current workspace changes relative to HEAD.',
    `Original command:\n${commandText}`,
    contextBlock,
    reviewTeamPromptBlock,
  ].join('\n\n');
}
