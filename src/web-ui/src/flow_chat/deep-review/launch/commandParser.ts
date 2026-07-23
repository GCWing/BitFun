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

const PROSE_SLASH_COMPOUNDS = new Set(['ui/ux', 'read/write']);
const RECOGNIZED_PROJECT_ROOTS = new Set([
  'src',
  'docs',
  'tests',
  'scripts',
  'packages',
  'apps',
  'crates',
  'bitfun-installer',
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
    .replace(/[`"']+$/, '')
    .replace(/[.,;!?]+$/, '')
    .replace(/:(?:\d+)(?::\d+)?$/, '');
}

interface ReviewFocusToken {
  value: string;
  quoted: boolean;
  start: number;
  end: number;
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
        start: match.index,
        end: match.index + match[0].length,
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

function looksLikeExplicitReviewPath(
  token: string,
  quoted = false,
  strict = false,
): boolean {
  const cleaned = cleanPotentialFileToken(token);
  const normalizedPath = normalizeReviewPath(cleaned);
  const basename = normalizedPath.split('/').at(-1)?.toLowerCase() ?? '';
  const lowerPath = normalizedPath.toLowerCase();
  if (
    cleaned.includes('://') ||
    (cleaned.includes('..') && !cleaned.startsWith('../')) ||
    (PROSE_SLASH_COMPOUNDS.has(lowerPath) && !quoted)
  ) {
    return false;
  }
  const hasRecognizedName =
    EXPLICIT_REVIEW_FILE_EXTENSIONS.has(getPathExtension(normalizedPath)) ||
    EXPLICIT_REVIEW_EXTENSIONLESS_FILES.has(basename) ||
    (basename.startsWith('.') && basename.length > 1);
  const isDotRelative = /^(?:\.\.?)[\\/]/.test(cleaned);
  const isAbsolute = /^[A-Za-z]:[\\/]/.test(cleaned) || /^[\\/]/.test(cleaned);
  const hasSeparator = /[\\/]/.test(cleaned);
  const startsAtProjectRoot =
    hasSeparator &&
    RECOGNIZED_PROJECT_ROOTS.has(lowerPath.split('/')[0]);
  const hasExplicitTrailingSlash = /[\\/]$/.test(cleaned);
  const hasStrongEvidence =
    hasRecognizedName ||
    isDotRelative ||
    isAbsolute ||
    startsAtProjectRoot ||
    hasExplicitTrailingSlash ||
    (quoted && hasSeparator);
  return (
    (
      hasStrongEvidence ||
      (!strict && hasSeparator)
    ) &&
    !normalizedPath.startsWith('-') &&
    normalizedPath.length > 0
  );
}

export function hasUnresolvedPathLikeReviewFocus(commandFocus: string): boolean {
  return extractUnresolvedPathLikeReviewFocusFragments(commandFocus).length > 0;
}

export function extractUnresolvedPathLikeReviewFocusFragments(
  commandFocus: string,
): string[] {
  const tokens = tokenizeReviewFocus(commandFocus).map((token) => ({
    ...token,
    value: cleanPotentialFileToken(token.value),
  }));
  return tokens.flatMap(({ value: token, quoted }, index) => {
    if (!token || looksLikeExplicitReviewPath(token, quoted, true)) return [];
    if (token.includes('://')) return [];
    if (isThreeDotRange(token)) return [token];
    const previousKeyword = tokens[index - 1]?.value.toLowerCase() ?? '';
    if (['commit', 'ref'].includes(token.toLowerCase())) {
      return [];
    }
    if (['commit', 'ref'].includes(previousKeyword)) {
      return isValidExplicitRef(token) ? [] : [token];
    }
    if (PROSE_SLASH_COMPOUNDS.has(token.toLowerCase())) return [];
    if (token.includes('..')) {
      return parseTwoDotRange(token) ? [] : [token];
    }
    const normalized = normalizeReviewPath(token);
    const basename = normalized.split('/').at(-1) ?? '';
    const unresolved = (
      quoted ||
      /[\\/]/.test(token) ||
      (basename.startsWith('.') && basename.length > 1) ||
      (basename.includes('.') && !/^v?\d+(?:\.\d+)+$/i.test(basename)) ||
      /^[A-Z][A-Z0-9]*(?:[_-][A-Z0-9]+)+$/.test(basename)
    );
    return unresolved ? [token] : [];
  });
}

export interface ExplicitReviewFilePathMatch {
  path: string;
  start: number;
  end: number;
}

export interface ExplicitReviewFilePathOptions {
  strict?: boolean;
}

export function extractExplicitReviewFilePathMatches(
  commandFocus: string,
  options: ExplicitReviewFilePathOptions = {},
): ExplicitReviewFilePathMatch[] {
  const tokens = tokenizeReviewFocus(commandFocus).map((token) => ({
    ...token,
    path: cleanPotentialFileToken(token.value),
  }));

  return tokens.flatMap(({ path, quoted, start, end }, index) => {
    if (!path || path.toLowerCase() === 'commit') return [];
    const previous = tokens[index - 1]?.path.toLowerCase();
    if (previous === 'commit' || previous === 'ref') return [];
    return looksLikeExplicitReviewPath(path, quoted, options.strict)
      ? [{ path, start, end }]
      : [];
  });
}

export function extractExplicitReviewFilePaths(
  commandFocus: string,
  options: ExplicitReviewFilePathOptions = {},
): string[] {
  return Array.from(new Set(
    extractExplicitReviewFilePathMatches(commandFocus, options).map(({ path }) => path),
  ));
}

export interface ReviewGitTargetMatch {
  source: string;
  target: string;
  start: number;
  end: number;
}

function parseTwoDotRange(token: string): [string, string] | null {
  if (token.startsWith('-') || token.includes('://')) return null;
  const separator = token.indexOf('..');
  if (
    separator <= 0 ||
    token.lastIndexOf('..') !== separator ||
    token[separator - 1] === '.' ||
    token[separator + 2] === '.'
  ) {
    return null;
  }
  const source = token.slice(0, separator);
  const target = token.slice(separator + 2);
  return isValidExplicitRef(source) && isValidExplicitRef(target)
    ? [source, target]
    : null;
}

function isThreeDotRange(token: string): boolean {
  const separator = token.indexOf('...');
  return separator > 0 && separator + 3 < token.length;
}

function isValidExplicitRef(ref: string): boolean {
  if (
    !ref ||
    ref === '@' ||
    ref.startsWith('-') ||
    ref.startsWith('/') ||
    ref.endsWith('/') ||
    ref.endsWith('.') ||
    ref.includes('://') ||
    ref.includes('..') ||
    ref.includes('@{') ||
    ref.includes('//') ||
    hasInvalidRefCharacter(ref)
  ) {
    return false;
  }
  return ref
    .split('/')
    .every((component) => !component.startsWith('.') && !component.endsWith('.lock'));
}

function hasInvalidRefCharacter(value: string): boolean {
  for (const character of value) {
    const codePoint = character.codePointAt(0);
    if (codePoint !== undefined && (codePoint <= 0x20 || codePoint === 0x7f)) {
      return true;
    }
  }
  return /[~^:?*[\]\\]/.test(value);
}

function isExplicitRefKeywordPosition(commandFocus: string, start: number): boolean {
  const prefix = commandFocus.slice(0, start).trimEnd();
  return (
    prefix.length === 0 ||
    prefix.toLowerCase() === 'review' ||
    /[,;:([{]$/.test(prefix) ||
    /(?:^|\s)(?:and|\u4ee5\u53ca|\u548c|\u4e0e)$/iu.test(prefix)
  );
}

export function extractReviewGitTargetMatches(
  commandFocus: string,
): ReviewGitTargetMatch[] {
  const tokens = tokenizeReviewFocus(commandFocus).map((token) => ({
    ...token,
    cleaned: cleanPotentialFileToken(token.value),
  }));
  const matches: ReviewGitTargetMatch[] = [];

  for (const token of tokens) {
    const range = parseTwoDotRange(token.cleaned);
    if (range) {
      matches.push({
        source: range[0],
        target: range[1],
        start: token.start,
        end: token.end,
      });
    }
  }

  for (let index = 0; index < tokens.length - 1; index += 1) {
    const keyword = tokens[index];
    if (
      !['commit', 'ref'].includes(keyword.cleaned.toLowerCase()) ||
      !isExplicitRefKeywordPosition(commandFocus, keyword.start)
    ) {
      continue;
    }
    const refToken = tokens[index + 1];
    if (!isValidExplicitRef(refToken.cleaned)) continue;
    matches.push({
      source: `${refToken.cleaned}^`,
      target: refToken.cleaned,
      start: keyword.start,
      end: refToken.end,
    });
  }

  return matches.sort((left, right) => left.start - right.start);
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
