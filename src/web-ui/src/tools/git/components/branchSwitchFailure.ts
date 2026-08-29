export type CheckoutOverwriteKind = 'tracked' | 'untracked';

export interface CheckoutOverwriteFailure {
  kind: CheckoutOverwriteKind;
  files: string[];
}

export interface BranchSwitchFileStats {
  additions: number;
  deletions: number;
}

const CHECKOUT_OVERWRITE_HEADERS: Array<{
  kind: CheckoutOverwriteKind;
  text: string;
}> = [
  {
    kind: 'tracked',
    text: 'your local changes to the following files would be overwritten by checkout:',
  },
  {
    kind: 'untracked',
    text: 'the following untracked working tree files would be overwritten by checkout:',
  },
];

const CHECKOUT_DIAGNOSTIC_END = /^(?:please\s|aborting\.?$|error:|fatal:)/i;

const uniquePaths = (paths: string[]): string[] => Array.from(new Set(
  paths.map(path => path.trim()).filter(Boolean),
));

/**
 * Extracts the machine-stable part of Git's English checkout diagnostic.
 *
 * Local Git commands pin `LC_ALL=C`, and the remote adapter returns the same
 * stderr payload. The leading desktop/service context is intentionally ignored
 * so this continues to work on either transport.
 */
export function parseCheckoutOverwriteFailure(
  error: string | null | undefined,
): CheckoutOverwriteFailure | null {
  if (!error?.trim()) return null;

  const lines = error.replace(/\r\n/g, '\n').split('\n');
  const headerIndex = lines.findIndex(line => {
    const lower = line.toLowerCase();
    return CHECKOUT_OVERWRITE_HEADERS.some(header => lower.includes(header.text));
  });
  if (headerIndex < 0) return null;

  const headerLine = lines[headerIndex].toLowerCase();
  const header = CHECKOUT_OVERWRITE_HEADERS.find(candidate => (
    headerLine.includes(candidate.text)
  ));
  if (!header) return null;

  const files: string[] = [];
  for (const line of lines.slice(headerIndex + 1)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    if (CHECKOUT_DIAGNOSTIC_END.test(trimmed)) break;
    files.push(trimmed);
  }

  return { kind: header.kind, files: uniquePaths(files) };
}

const unquoteGitPath = (path: string): string => {
  const trimmed = path.trim();
  if (!(trimmed.startsWith('"') && trimmed.endsWith('"'))) return trimmed;

  // Git only quotes paths when core.quotePath or special characters require
  // it. JSON understands the common escapes Git emits; if an octal escape is
  // present, keeping the quoted spelling is safer than inventing a path.
  try {
    return JSON.parse(trimmed) as string;
  } catch {
    return trimmed.slice(1, -1);
  }
};

const pathFromDiffSection = (section: string): string | null => {
  const lines = section.split('\n');
  const newPathLine = lines.find(line => line.startsWith('+++ '));
  const oldPathLine = lines.find(line => line.startsWith('--- '));
  const candidate = newPathLine?.slice(4).trim() === '/dev/null'
    ? oldPathLine?.slice(4).trim()
    : newPathLine?.slice(4).trim();

  if (!candidate || candidate === '/dev/null') return null;
  const path = unquoteGitPath(candidate);
  return path.startsWith('a/') || path.startsWith('b/') ? path.slice(2) : path;
};

/** Parses per-file line counts from a normal unified `git diff` payload. */
export function parseUnifiedDiffStats(
  diffOutput: string,
): Map<string, BranchSwitchFileStats> {
  const result = new Map<string, BranchSwitchFileStats>();
  if (!diffOutput.trim()) return result;

  const sections = diffOutput.split(/^diff --git /m).filter(Boolean);
  for (const rawSection of sections) {
    const section = `diff --git ${rawSection}`;
    const path = pathFromDiffSection(section);
    if (!path) continue;

    let additions = 0;
    let deletions = 0;
    for (const line of section.split('\n')) {
      if (line.startsWith('+') && !line.startsWith('+++')) additions += 1;
      if (line.startsWith('-') && !line.startsWith('---')) deletions += 1;
    }
    result.set(path, { additions, deletions });
  }

  return result;
}

export function mergeBranchSwitchFileStats(
  ...groups: Array<Map<string, BranchSwitchFileStats>>
): Map<string, BranchSwitchFileStats> {
  const merged = new Map<string, BranchSwitchFileStats>();
  for (const group of groups) {
    for (const [path, stats] of group) {
      const current = merged.get(path) ?? { additions: 0, deletions: 0 };
      merged.set(path, {
        additions: current.additions + stats.additions,
        deletions: current.deletions + stats.deletions,
      });
    }
  }
  return merged;
}

export function fallbackChangedPaths(state: {
  staged?: Array<{ path: string }>;
  unstaged?: Array<{ path: string }>;
  untracked?: string[];
  conflicts?: string[];
} | null | undefined): string[] {
  if (!state) return [];
  return uniquePaths([
    ...(state.staged ?? []).map(file => file.path),
    ...(state.unstaged ?? []).map(file => file.path),
    ...(state.untracked ?? []),
    ...(state.conflicts ?? []),
  ]);
}
