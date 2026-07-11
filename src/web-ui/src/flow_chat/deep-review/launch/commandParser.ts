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
  '.proto',
  '.graphql',
  '.sql',
  '.vue',
  '.svelte',
  '.gradle',
  '.ini',
  '.cfg',
  '.conf',
  '.mod',
  '.sum',
  '.bazel',
]);

const EXPLICIT_REVIEW_EXTENSIONLESS_FILES = new Set([
  'dockerfile',
  'makefile',
  'procfile',
  'readme',
  'license',
  'build',
  'workspace',
  'justfile',
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
    .replace(/[`"',;]+$/, '')
    .replace(/:(?:\d+)(?::\d+)?$/, '');
}

interface ReviewFocusToken {
  value: string;
  quoted: boolean;
}

function tokenizeReviewFocus(commandFocus: string): ReviewFocusToken[] {
  const tokens: ReviewFocusToken[] = [];
  const tokenPattern = /`([^`]+)`|"([^"]+)"|'([^']+)'|(\S+)/g;
  let match: RegExpExecArray | null;
  while ((match = tokenPattern.exec(commandFocus)) !== null) {
    const token = match[1] ?? match[2] ?? match[3] ?? match[4];
    if (token) {
      tokens.push({
        value: token,
        quoted: match[4] === undefined,
      });
    }
  }
  return tokens;
}

function getPathExtension(path: string): string {
  const lastSlash = path.lastIndexOf('/');
  const lastDot = path.lastIndexOf('.');
  if (lastDot <= lastSlash) {
    return '';
  }
  return path.slice(lastDot).toLowerCase();
}

function looksLikeExplicitReviewPath(token: string): boolean {
  const cleaned = cleanPotentialFileToken(token);
  const normalizedPath = normalizeReviewPath(cleaned);
  const basename = normalizedPath.split('/').at(-1)?.toLowerCase() ?? '';
  if (
    cleaned.includes('://') ||
    (cleaned.includes('..') && !cleaned.startsWith('../'))
  ) {
    return false;
  }
  return (
    (
      EXPLICIT_REVIEW_FILE_EXTENSIONS.has(getPathExtension(normalizedPath)) ||
      EXPLICIT_REVIEW_EXTENSIONLESS_FILES.has(basename) ||
      (basename.startsWith('.') && basename.length > 1) ||
      cleaned.startsWith('./') ||
      cleaned.startsWith('../') ||
      /[\\/]/.test(cleaned) ||
      /[\\/]$/.test(cleaned)
    ) &&
    !normalizedPath.startsWith('-') &&
    normalizedPath.length > 0
  );
}

export function hasUnresolvedPathLikeReviewFocus(commandFocus: string): boolean {
  const tokens = tokenizeReviewFocus(commandFocus).map((token) => ({
    value: cleanPotentialFileToken(token.value),
    quoted: token.quoted,
  }));
  return tokens.some(({ value: token, quoted }, index) => {
    if (!token || looksLikeExplicitReviewPath(token)) return false;
    if (
      token.toLowerCase() === 'commit' ||
      tokens[index - 1]?.value.toLowerCase() === 'commit'
    ) {
      return false;
    }
    if (token.includes('://') || token.includes('..')) return false;
    const normalized = normalizeReviewPath(token);
    const basename = normalized.split('/').at(-1) ?? '';
    return (
      quoted ||
      /[\\/]/.test(token) ||
      (basename.startsWith('.') && basename.length > 1) ||
      (basename.includes('.') && !/^v?\d+(?:\.\d+)+$/i.test(basename)) ||
      /^[A-Z][A-Z0-9]*(?:[_-][A-Z0-9]+)+$/.test(basename)
    );
  });
}

export function extractExplicitReviewFilePaths(commandFocus: string): string[] {
  const tokens = tokenizeReviewFocus(commandFocus).map((token) => cleanPotentialFileToken(token.value));
  const paths = tokens.filter((token, index) => {
    if (!token) return false;
    if (token.toLowerCase() === 'commit') return false;
    if (tokens[index - 1]?.toLowerCase() === 'commit') return false;
    return looksLikeExplicitReviewPath(token);
  });

  return Array.from(new Set(paths));
}

export function parseSlashCommandGitTarget(commandFocus: string): GitChangedFilesParams | null {
  const tokens = tokenizeReviewFocus(commandFocus)
    .map((token) => cleanPotentialFileToken(token.value))
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
