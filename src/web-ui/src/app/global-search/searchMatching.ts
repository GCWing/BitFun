import type { GlobalSearchScope } from './types';

const WORD_BOUNDARY = /[\s._/\\:-]+/u;

export interface ParsedGlobalSearchQuery {
  query: string;
  scope: GlobalSearchScope;
  scopeForcedByPrefix: boolean;
}
export function parseGlobalSearchQuery(
  rawQuery: string,
  selectedScope: GlobalSearchScope,
): ParsedGlobalSearchQuery {
  const trimmed = rawQuery.trim();
  if (trimmed.startsWith('>')) {
    return {
      query: trimmed.slice(1).trim(),
      scope: 'actions',
      scopeForcedByPrefix: true,
    };
  }
  return { query: trimmed, scope: selectedScope, scopeForcedByPrefix: false };
}

function isSubsequence(query: string, candidate: string): boolean {
  let queryIndex = 0;
  for (let index = 0; index < candidate.length && queryIndex < query.length; index += 1) {
    if (candidate[index] === query[queryIndex]) {
      queryIndex += 1;
    }
  }
  return queryIndex === query.length;
}

/**
 * Small deterministic matcher shared by in-memory providers.
 * Backend content providers keep ownership of their own relevance scores.
 */
export function scoreTextMatch(query: string, fields: Array<string | null | undefined>): number {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  if (!normalizedQuery) return 70;

  let best = 0;
  for (const rawField of fields) {
    const field = rawField?.trim().toLocaleLowerCase();
    if (!field) continue;
    if (field === normalizedQuery) {
      best = Math.max(best, 100);
      continue;
    }
    if (field.startsWith(normalizedQuery)) {
      best = Math.max(best, 94);
      continue;
    }
    const words = field.split(WORD_BOUNDARY).filter(Boolean);
    if (words.some((word) => word.startsWith(normalizedQuery))) {
      best = Math.max(best, 88);
      continue;
    }
    if (field.includes(normalizedQuery)) {
      best = Math.max(best, 80);
      continue;
    }

    const queryTokens = normalizedQuery.split(WORD_BOUNDARY).filter(Boolean);
    if (queryTokens.length > 1 && queryTokens.every((token) => field.includes(token))) {
      best = Math.max(best, 74);
      continue;
    }

    const acronym = words.map((word) => word[0]).join('');
    if (normalizedQuery.length > 1 && isSubsequence(normalizedQuery, acronym)) {
      best = Math.max(best, 66);
    }
  }
  return best;
}
