import type {
  GitChangedFile,
  GitChangedFilesParams,
  GitStatus,
} from '@/infrastructure/api/service-api/GitAPI';
import { normalizeReviewPath } from '@/shared/services/reviewTargetClassifier';
import { DEEP_REVIEW_COMMAND_RE } from '../../utils/deepReviewConstants';

export const DEEP_REVIEW_SLASH_COMMAND = '/DeepReview';

const EXPLICIT_REVIEW_FILE_EXTENSIONS = new Set([
  '.ts',
  '.tsx',
  '.js',
  '.jsx',
  '.rs',
  '.json',
  '.scss',
  '.css',
  '.md',
  '.toml',
  '.yaml',
  '.yml',
]);

export function isDeepReviewSlashCommand(commandText: string): boolean {
  return DEEP_REVIEW_COMMAND_RE.test(commandText.trim());
}

export function getDeepReviewCommandFocus(commandText: string): string {
  return commandText.trim().replace(/^\/DeepReview\b/, '').trim();
}

function cleanPotentialFileToken(token: string): string {
  return token
    .trim()
    .replace(/^[`"']+/, '')
    .replace(/[`"',;:]+$/, '');
}

function getPathExtension(path: string): string {
  const lastSlash = path.lastIndexOf('/');
  const lastDot = path.lastIndexOf('.');
  if (lastDot <= lastSlash) {
    return '';
  }
  return path.slice(lastDot);
}

function looksLikeExplicitReviewPath(token: string): boolean {
  const normalizedPath = normalizeReviewPath(token);
  return (
    normalizedPath.includes('/') &&
    !normalizedPath.startsWith('-') &&
    EXPLICIT_REVIEW_FILE_EXTENSIONS.has(getPathExtension(normalizedPath))
  );
}

export function extractExplicitReviewFilePaths(commandFocus: string): string[] {
  const paths = commandFocus
    .split(/\s+/)
    .map(cleanPotentialFileToken)
    .filter(Boolean)
    .filter(looksLikeExplicitReviewPath);

  return Array.from(new Set(paths));
}

export function parseSlashCommandGitTarget(commandFocus: string): GitChangedFilesParams | null {
  const tokens = commandFocus
    .split(/\s+/)
    .map(cleanPotentialFileToken)
    .filter(Boolean);

  const commitKeywordIndex = tokens.findIndex((token) => token.toLowerCase() === 'commit');
  const commitRef = commitKeywordIndex >= 0 ? tokens[commitKeywordIndex + 1] : undefined;
  if (commitRef && !commitRef.startsWith('-')) {
    return {
      source: `${commitRef}^`,
      target: commitRef,
    };
  }

  const rangeToken = tokens.find((token) => {
    if (token.startsWith('-') || !token.includes('..')) {
      return false;
    }

    const parts = token.split('..');
    return parts.length === 2 && Boolean(parts[0]) && Boolean(parts[1]);
  });

  if (!rangeToken) {
    return null;
  }

  const [source, target] = rangeToken.split('..');
  return { source, target };
}

export function collectChangedFilePaths(changedFiles: GitChangedFile[]): string[] {
  return Array.from(
    new Set(
      changedFiles
        .flatMap((file) => [file.path, file.old_path])
        .filter((path): path is string => Boolean(path)),
    ),
  );
}

export function collectWorkspaceDiffFilePaths(status: GitStatus): string[] {
  return Array.from(
    new Set([
      ...status.staged.map((file) => file.path),
      ...status.unstaged.map((file) => file.path),
      ...status.untracked,
      ...status.conflicts,
    ].filter(Boolean)),
  );
}
