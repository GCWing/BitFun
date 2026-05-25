const GOAL_COMMAND_PATTERN = /^\/goal(?:\s+(.*))?$/i;

export function parseGoalCommand(message: string): { userHint?: string } | null {
  const trimmed = message.trim();
  const match = trimmed.match(GOAL_COMMAND_PATTERN);
  if (!match) {
    return null;
  }
  const userHint = match[1]?.trim();
  return { userHint: userHint || undefined };
}

export function isGoalSlashCommand(message: string): boolean {
  return GOAL_COMMAND_PATTERN.test(message.trim());
}
