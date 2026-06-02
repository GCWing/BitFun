/**
 * Slash-command matching helpers for the chat composer.
 *
 * Mirrors Claude Code 2.1.147: a slash command followed by trailing
 * whitespace (including tabs and newlines) is still recognised as that
 * command, never "an unknown command". Trailing `tab` and `newline` are
 * valid word boundaries in the strict sense, so a literal-prefix check
 * (`text.startsWith('/btw')`) already happens to accept them, but the
 * same check also accidentally accepts `/btwextra`, `/goals`, etc. —
 * commands with the same prefix. The helpers in this module enforce a
 * proper word boundary so e.g. `/btwextra` is NOT treated as `/btw`.
 *
 * Use these in preference to `text.startsWith('/xxx')` everywhere the
 * chat input decides which slash command a typed line is.
 */

const COMMAND_BOUNDARY_RE = /^(\/[a-zA-Z][\w:-]*)(?=\s|$)/;

/**
 * Returns the matched command token (lowercase, with leading `/`) if
 * `text` begins with a recognised slash command, otherwise null.
 *
 * A "recognised" command is one whose name starts with a `/` followed by
 * a letter and contains only word characters (`[a-zA-Z0-9_:-]`) up to
 * the first whitespace or end-of-string boundary. This is the same
 * boundary rule Claude Code uses to detect slash commands.
 *
 * Examples:
 *   matchesSlashCommand('/goal focus the bug')  -> '/goal'
 *   matchesSlashCommand('/goal\t')              -> '/goal'   (2.1.147)
 *   matchesSlashCommand('/goal\nnext line')     -> '/goal'   (2.1.147)
 *   matchesSlashCommand('/goals')               -> '/goals'  (NOT '/goal')
 *   matchesSlashCommand('/btwextra')            -> '/btwextra'
 *   matchesSlashCommand('hello')                -> null
 *   matchesSlashCommand('')                     -> null
 */
export function matchesSlashCommand(text: string): string | null {
  if (typeof text !== 'string' || text.length === 0) {
    return null;
  }
  // Do NOT trim leading whitespace here — the caller is expected to have
  // already done so (see e.g. `inputState.value.trim()` in ChatInput.tsx).
  // A line that is only whitespace or starts with anything other than `/`
  // is not a slash command.
  if (!text.startsWith('/')) {
    return null;
  }
  const match = text.match(COMMAND_BOUNDARY_RE);
  return match ? match[1].toLowerCase() : null;
}

/**
 * Convenience predicate: does `text` start with the given slash command?
 *
 * Examples:
 *   isSlashCommand('/btw hello', '/btw')    -> true
 *   isSlashCommand('/btw', '/btw')          -> true
 *   isSlashCommand('/btw\t', '/btw')        -> true
 *   isSlashCommand('/btwextra', '/btw')     -> false
 *   isSlashCommand('not a command', '/btw') -> false
 */
export function isSlashCommand(text: string, command: string): boolean {
  if (typeof command !== 'string' || !command.startsWith('/')) {
    return false;
  }
  const matched = matchesSlashCommand(text);
  return matched === command.toLowerCase();
}

/**
 * Strip the leading `/cmd` token (and any whitespace right after it) from
 * `text`, returning the remainder. If `text` does not start with `command`,
 * the original string is returned unchanged. All leading whitespace after
 * the command is consumed (including tabs and newlines — the 2.1.147
 * case) so callers receive just the argument.
 *
 * Examples:
 *   stripSlashCommand('/btw\tquestion', '/btw')  -> 'question'
 *   stripSlashCommand('/btw  question', '/btw')  -> 'question'
 *   stripSlashCommand('/btw', '/btw')            -> ''
 *   stripSlashCommand('/btwextra', '/btw')       -> '/btwextra' (unchanged)
 */
export function stripSlashCommand(text: string, command: string): string {
  if (!isSlashCommand(text, command)) {
    return text;
  }
  // text starts with /cmd followed by either whitespace or end-of-string.
  // Consume the command and any leading whitespace (including tab/newline,
  // the 2.1.147 case) so callers see only the argument.
  const escaped = command.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return text.replace(new RegExp(`^${escaped}[\\s]*`), '');
}
