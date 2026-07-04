const COMMAND_BOUNDARY_RE = /^(\/[A-Za-z][\w:-]*)(?=\s|$)/;

export function isSlashCommandPickerQuery(query: string): boolean {
  return typeof query === 'string' && !query.includes('/');
}

export function getSlashCommandPickerQuery(text: string): string | null {
  if (typeof text !== 'string' || !text.startsWith('/')) {
    return null;
  }

  const query = text.slice(1);
  if (/\s/.test(query) || !isSlashCommandPickerQuery(query)) {
    return null;
  }

  return query.toLowerCase();
}

export function matchesSlashCommand(text: string): string | null {
  if (typeof text !== 'string' || text.length === 0 || !text.startsWith('/')) {
    return null;
  }

  const match = text.match(COMMAND_BOUNDARY_RE);
  return match ? match[1].toLowerCase() : null;
}

export function isSlashCommand(text: string, command: string): boolean {
  if (typeof command !== 'string' || !command.startsWith('/')) {
    return false;
  }

  return matchesSlashCommand(text) === command.toLowerCase();
}

export function stripSlashCommand(text: string, command: string): string {
  const normalizedCommand =
    typeof command === 'string' && command.startsWith('/')
      ? command.toLowerCase()
      : null;

  if (!normalizedCommand || matchesSlashCommand(text) !== normalizedCommand) {
    return text;
  }

  return text.slice(normalizedCommand.length).replace(/^\s*/, '');
}
