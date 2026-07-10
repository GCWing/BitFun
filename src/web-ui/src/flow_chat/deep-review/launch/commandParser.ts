import type {
  GitChangedFile,
  GitChangedFilesParams,
  GitStatus,
} from '@/infrastructure/api/service-api/GitAPI';
import { normalizeReviewPath } from '@/shared/services/reviewTargetClassifier';
import {
  DEEP_REVIEW_COMMAND_RE,
  DEEP_REVIEW_COMPAT_COMMAND_PREFIX_RE,
  REVIEW_COMMAND_PREFIX_RE,
  REVIEW_COMMAND_RE,
  REVIEW_STRICT_COMMAND_PREFIX_RE,
  REVIEW_STRICT_SLASH_COMMAND,
} from '../../utils/deepReviewConstants';

export const DEEP_REVIEW_SLASH_COMMAND = REVIEW_STRICT_SLASH_COMMAND;

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
  '.lock',
  '.txt',
  '.sh',
  '.py',
  '.go',
  '.java',
  '.kt',
  '.swift',
  '.c',
  '.h',
  '.cpp',
  '.hpp',
  '.xml',
  '.ftl',
]);

export function isDeepReviewSlashCommand(commandText: string): boolean {
  return DEEP_REVIEW_COMMAND_RE.test(commandText.trim());
}

export function isReviewSlashCommand(commandText: string): boolean {
  return REVIEW_COMMAND_RE.test(commandText.trim());
}

export function getReviewSlashCommandIntent(commandText: string): 'adaptive' | 'strict' {
  return isDeepReviewSlashCommand(commandText) ? 'strict' : 'adaptive';
}

export function getDeepReviewCommandFocus(commandText: string): string {
  return commandText
    .trim()
    .replace(REVIEW_STRICT_COMMAND_PREFIX_RE, '')
    .replace(DEEP_REVIEW_COMPAT_COMMAND_PREFIX_RE, '')
    .replace(REVIEW_COMMAND_PREFIX_RE, '')
    .trim();
}

function cleanPotentialFileToken(token: string): string {
  return token
    .trim()
    .replace(/^[`"']+/, '')
    .replace(/[`"',;:]+$/, '');
}

function tokenizeReviewFocus(commandFocus: string): string[] {
  const tokens: string[] = [];
  const tokenPattern = /`([^`]+)`|"([^"]+)"|'([^']+)'|(\S+)/g;
  let match: RegExpExecArray | null;
  while ((match = tokenPattern.exec(commandFocus)) !== null) {
    const token = match[1] ?? match[2] ?? match[3] ?? match[4];
    if (token) tokens.push(token);
  }
  return tokens;
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
  const cleaned = cleanPotentialFileToken(token);
  const normalizedPath = normalizeReviewPath(token);
  return (
    (
      EXPLICIT_REVIEW_FILE_EXTENSIONS.has(getPathExtension(normalizedPath)) ||
      cleaned.startsWith('./') ||
      cleaned.startsWith('../') ||
      /[\\/]$/.test(cleaned)
    ) &&
    !normalizedPath.startsWith('-') &&
    normalizedPath.length > 0
  );
}

export function extractExplicitReviewFilePaths(commandFocus: string): string[] {
  const paths = tokenizeReviewFocus(commandFocus)
    .map(cleanPotentialFileToken)
    .filter(Boolean)
    .filter(looksLikeExplicitReviewPath);

  return Array.from(new Set(paths));
}

export function parseSlashCommandGitTarget(commandFocus: string): GitChangedFilesParams | null {
  const tokens = tokenizeReviewFocus(commandFocus)
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
    new Set(changedFiles.map((file) => file.path).filter(Boolean)),
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
