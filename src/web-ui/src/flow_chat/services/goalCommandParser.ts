/** Matches /goal with optional objective text (including newlines). Rejects `/goalie`. */
const GOAL_COMMAND_PATTERN = /^\/goal(?=\s|$)(?:\s*([\s\S]*))?$/i;

export type GoalCommandAction =
  | { kind: 'menu' }
  | { kind: 'set'; objective: string }
  | { kind: 'edit' }
  | { kind: 'clear' }
  | { kind: 'pause' }
  | { kind: 'resume' };

function parseGoalControlCommand(args: string): GoalCommandAction | null {
  if (!args || /\r?\n/.test(args)) {
    return null;
  }
  const control = args.toLowerCase();
  if (control === 'clear') {
    return { kind: 'clear' };
  }
  if (control === 'pause') {
    return { kind: 'pause' };
  }
  if (control === 'resume') {
    return { kind: 'resume' };
  }
  if (control === 'edit') {
    return { kind: 'edit' };
  }
  return null;
}

export function parseGoalCommand(message: string): GoalCommandAction | null {
  const trimmed = message.trim();
  const match = trimmed.match(GOAL_COMMAND_PATTERN);
  if (!match) {
    return null;
  }

  const args = match[1]?.trim() ?? '';
  if (!args) {
    return { kind: 'menu' };
  }

  const control = parseGoalControlCommand(args);
  if (control) {
    return control;
  }

  return { kind: 'set', objective: args };
}

export function isGoalSlashCommand(message: string): boolean {
  return GOAL_COMMAND_PATTERN.test(message.trim());
}
