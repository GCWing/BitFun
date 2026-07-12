import {
  extractExplicitReviewFilePathMatches,
  extractExplicitReviewFilePaths,
  extractReviewGitTargetMatches,
  extractUnresolvedPathLikeReviewFocusFragments,
} from './commandParser';

export type ReviewSubjectCandidate =
  | {
      kind: 'issue';
      id: string;
      web_url: string;
      host: string;
      project_path: string;
      issue_id: string;
    }
  | {
      kind: 'pull_request';
      id: string;
      web_url: string;
      host: string;
      project_path: string;
      pull_request_id: string;
    }
  | {
      kind: 'git_range';
      id: string;
      source_ref: string;
      target_ref: string;
    }
  | {
      kind: 'workspace';
      id: string;
      workspace_path: string;
    }
  | {
      kind: 'explicit_files';
      id: string;
      paths: string[];
    }
  | {
      kind: 'external_reference';
      id: string;
      url: string;
    };

const URL_TOKEN = /https?:\/\/[^\s`"'<>]+/giu;
const GITHUB_PROVIDER_RESOURCE = /^https?:\/\/[^/?#\s]+\/[^/?#\s]+\/[^/?#\s]+\/(?:issues|pull)\/\d+/i;
const GITLAB_PROVIDER_RESOURCE = /^https?:\/\/[^/?#\s]+\/(?:[^/?#\s]+\/)+-\/(?:issues|merge_requests)\/\d+/i;
const PROVIDER_ADJACENT_CJK = /^[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}\u3000-\u303F\uFF00-\uFFEF]/u;

type ReviewProvider = 'github' | 'gitlab';

export interface ReviewSubjectCandidateExtractionOptions {
  trustedProviderHosts?: Readonly<Record<string, ReviewProvider>>;
}

type CandidateWithoutId = ReviewSubjectCandidate extends infer Candidate
  ? Candidate extends ReviewSubjectCandidate
    ? Omit<Candidate, 'id'>
    : never
  : never;

declare const reviewSubjectIdentityBrand: unique symbol;

/**
 * A launch-local normalized subject identity. Candidate mentions with the same
 * identity collapse to the first textual occurrence before IDs are assigned.
 */
type ReviewSubjectIdentity = string & {
  readonly [reviewSubjectIdentityBrand]: true;
};

interface CandidateSpan {
  start: number;
  end: number;
  candidate: CandidateWithoutId;
  identity: ReviewSubjectIdentity;
}

export interface ReviewSubjectCandidateExtraction {
  candidates: ReviewSubjectCandidate[];
  remainingFocus: string;
  unparsedFragments: string[];
}

function asSubjectIdentity(value: string): ReviewSubjectIdentity {
  return value as ReviewSubjectIdentity;
}

export function trimReviewUrlToken(value: string): string {
  const openerForCloser: Record<string, string> = {
    ')': '(',
    ']': '[',
    '}': '{',
  };
  const openers = new Set(Object.values(openerForCloser));
  const stack: string[] = [];
  const unmatchedClosers = new Set<number>();

  for (let index = 0; index < value.length; index += 1) {
    const character = value[index];
    if (openers.has(character)) {
      stack.push(character);
      continue;
    }
    const opener = openerForCloser[character];
    if (!opener) continue;
    if (stack.at(-1) === opener) {
      stack.pop();
    } else {
      unmatchedClosers.add(index);
    }
  }

  let end = value.length;
  while (
    end > 0 &&
    (/[.,;:!?]/.test(value[end - 1]) || unmatchedClosers.has(end - 1))
  ) {
    end -= 1;
  }
  return value.slice(0, end);
}

function normalizeProviderHost(host: string): string | null {
  try {
    return new URL(`https://${host}`).hostname.toLowerCase().replace(/\.$/, '');
  } catch {
    return null;
  }
}

function trustedProviders(
  options: ReviewSubjectCandidateExtractionOptions,
): Map<string, ReviewProvider> {
  const providers = new Map<string, ReviewProvider>([
    ['github.com', 'github'],
    ['gitlab.com', 'gitlab'],
  ]);
  for (const [host, provider] of Object.entries(options.trustedProviderHosts ?? {})) {
    const normalized = normalizeProviderHost(host);
    if (normalized) providers.set(normalized, provider);
  }
  return providers;
}

function decodeSafePathSegments(pathname: string): string[] | null {
  const rawSegments = pathname.split('/').slice(1);
  if (rawSegments.at(-1) === '') rawSegments.pop();
  if (rawSegments.some((segment) => segment.length === 0)) return null;

  const decoded: string[] = [];
  for (const segment of rawSegments) {
    try {
      const value = decodeURIComponent(segment);
      if (
        value === '.' ||
        value === '..' ||
        /[\\/]/.test(value) ||
        /[?#]/.test(value) ||
        /\p{White_Space}/u.test(value) ||
        /[\u0000-\u001F\u007F]/.test(value) ||
        /%[0-9A-F]{2}/i.test(value)
      ) {
        return null;
      }
      decoded.push(value);
    } catch {
      return null;
    }
  }
  return decoded;
}

function externalReference(rawUrl: string): CandidateWithoutId {
  return {
    kind: 'external_reference',
    url: rawUrl,
  };
}

function classifyProviderUrl(
  rawUrl: string,
  providers: Map<string, ReviewProvider>,
): CandidateWithoutId {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return externalReference(rawUrl);
  }
  if (url.username || url.password) return externalReference(rawUrl);

  const host = url.hostname.toLowerCase().replace(/\.$/, '');
  const provider = providers.get(host);
  if (!provider) return externalReference(rawUrl);
  const segments = decodeSafePathSegments(url.pathname);
  if (!segments) return externalReference(rawUrl);

  if (provider === 'github' && segments.length === 4) {
    const [owner, repository, resource, id] = segments;
    if (/^\d+$/.test(id) && resource === 'issues') {
      return {
        kind: 'issue',
        web_url: rawUrl,
        host,
        project_path: `${owner}/${repository}`,
        issue_id: id,
      };
    }
    if (/^\d+$/.test(id) && resource === 'pull') {
      return {
        kind: 'pull_request',
        web_url: rawUrl,
        host,
        project_path: `${owner}/${repository}`,
        pull_request_id: id,
      };
    }
  }

  if (provider === 'gitlab') {
    const delimiter = segments.lastIndexOf('-');
    const project = segments.slice(0, delimiter);
    const resource = segments[delimiter + 1];
    const id = segments[delimiter + 2];
    if (
      delimiter >= 2 &&
      delimiter + 3 === segments.length &&
      /^\d+$/.test(id)
    ) {
      if (resource === 'issues') {
        return {
          kind: 'issue',
          web_url: rawUrl,
          host,
          project_path: project.join('/'),
          issue_id: id,
        };
      }
      if (resource === 'merge_requests') {
        return {
          kind: 'pull_request',
          web_url: rawUrl,
          host,
          project_path: project.join('/'),
          pull_request_id: id,
        };
      }
    }
  }

  return externalReference(rawUrl);
}

function candidateIdentity(candidate: CandidateWithoutId): ReviewSubjectIdentity {
  switch (candidate.kind) {
    case 'issue':
      return asSubjectIdentity(
        `issue:${candidate.host}:${candidate.project_path}:${candidate.issue_id}`,
      );
    case 'pull_request':
      return asSubjectIdentity(
        `pull-request:${candidate.host}:${candidate.project_path}:${candidate.pull_request_id}`,
      );
    case 'git_range':
      return asSubjectIdentity(`git:${candidate.source_ref}..${candidate.target_ref}`);
    case 'explicit_files':
      return asSubjectIdentity(`files:${candidate.paths.join('\u0000')}`);
    case 'external_reference': {
      let normalized = candidate.url;
      try {
        normalized = new URL(candidate.url).href;
      } catch {
        // Keep the scanned URL when URL normalization is unavailable.
      }
      return asSubjectIdentity(`external:${normalized}`);
    }
    case 'workspace':
      return asSubjectIdentity(`workspace:${candidate.workspace_path}`);
  }
}

function isolateKnownProviderResource(
  token: string,
  providers: Map<string, ReviewProvider>,
): string {
  const authority = token.match(/^https?:\/\/[^/?#\s]+/iu)?.[0];
  if (!authority) return token;

  let authorityUrl: URL;
  try {
    authorityUrl = new URL(authority);
  } catch {
    return token;
  }
  if (authorityUrl.username || authorityUrl.password) return token;

  const host = authorityUrl.hostname.toLowerCase().replace(/\.$/, '');
  const provider = providers.get(host);
  const match = provider === 'github'
    ? token.match(GITHUB_PROVIDER_RESOURCE)
    : provider === 'gitlab'
      ? token.match(GITLAB_PROVIDER_RESOURCE)
      : null;
  if (!match) return token;
  const suffix = token.slice(match[0].length);
  return PROVIDER_ADJACENT_CJK.test(suffix) ? match[0] : token;
}

function extractUrlSpans(
  focus: string,
  options: ReviewSubjectCandidateExtractionOptions,
): CandidateSpan[] {
  const providers = trustedProviders(options);
  return Array.from(focus.matchAll(URL_TOKEN), (match) => {
    const scanned = isolateKnownProviderResource(match[0], providers);
    const rawUrl = trimReviewUrlToken(scanned);
    const candidate = classifyProviderUrl(rawUrl, providers);
    const start = match.index;
    return {
      start,
      end: start + scanned.length,
      candidate,
      identity: candidateIdentity(candidate),
    };
  }).filter(({ candidate }) => (
    candidate.kind !== 'external_reference' || candidate.url.length > 0
  ));
}

function stripSpans(
  focus: string,
  spans: Array<Pick<CandidateSpan, 'start' | 'end'>>,
): string {
  const orderedSpans = [...spans].sort((left, right) => left.start - right.start);
  const remaining: string[] = [];
  let cursor = 0;
  for (const { start, end } of orderedSpans) {
    if (end <= cursor) continue;
    remaining.push(focus.slice(cursor, Math.max(cursor, start)));
    cursor = end;
  }
  remaining.push(focus.slice(cursor));
  return remaining
    .join(' ')
    .replace(/(^|\s)[,;:]+(?=\s|$)/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function distinctCandidateSpans(spans: CandidateSpan[]): CandidateSpan[] {
  const seen = new Set<ReviewSubjectIdentity>();
  return [...spans]
    .sort((left, right) => left.start - right.start)
    .filter(({ identity }) => {
      if (seen.has(identity)) return false;
      seen.add(identity);
      return true;
    });
}

export function extractReviewSubjectCandidates(
  focus: string,
  workspacePath?: string,
  options: ReviewSubjectCandidateExtractionOptions = {},
): ReviewSubjectCandidateExtraction {
  const urlSpans = extractUrlSpans(focus, options);
  const gitSpans: CandidateSpan[] = extractReviewGitTargetMatches(focus).map((match) => {
    const candidate: CandidateWithoutId = {
      kind: 'git_range',
      source_ref: match.source,
      target_ref: match.target,
    };
    return {
      start: match.start,
      end: match.end,
      candidate,
      identity: candidateIdentity(candidate),
    };
  });
  const explicitFileMatches = extractExplicitReviewFilePathMatches(focus, {
    strict: true,
  });
  const explicitPaths = extractExplicitReviewFilePaths(focus, { strict: true });
  const candidateSpans: CandidateSpan[] = [...urlSpans, ...gitSpans];

  if (explicitPaths.length > 0) {
    const candidate: CandidateWithoutId = {
      kind: 'explicit_files',
      paths: explicitPaths,
    };
    candidateSpans.push({
      start: Math.min(...explicitFileMatches.map(({ start }) => start)),
      end: Math.max(...explicitFileMatches.map(({ end }) => end)),
      candidate,
      identity: candidateIdentity(candidate),
    });
  }

  const distinctSpans = distinctCandidateSpans(candidateSpans);
  const candidates: ReviewSubjectCandidate[] = distinctSpans.map(
    ({ candidate }, index) => ({
      ...candidate,
      id: `candidate-${index + 1}`,
    }) as ReviewSubjectCandidate,
  );
  const unparsedFragments = Array.from(new Set(
    extractUnresolvedPathLikeReviewFocusFragments(focus),
  ));
  if (
    candidates.length === 0 &&
    unparsedFragments.length === 0 &&
    workspacePath?.trim()
  ) {
    candidates.push({
      kind: 'workspace',
      id: 'candidate-1',
      workspace_path: workspacePath,
    });
  }

  return {
    candidates,
    remainingFocus: stripSpans(focus, [
      ...urlSpans,
      ...gitSpans,
      ...explicitFileMatches,
    ]),
    unparsedFragments,
  };
}
