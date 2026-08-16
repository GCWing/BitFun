import {
  GLOBAL_SEARCH_GROUP_ORDER,
  type GlobalSearchItem,
  type GlobalSearchProvider,
  type GlobalSearchProviderDiagnostic,
  type GlobalSearchProviderStatus,
  type GlobalSearchRequest,
  type GlobalSearchSnapshot,
} from './types';

function isAbortError(error: unknown): boolean {
  return typeof error === 'object'
    && error !== null
    && 'name' in error
    && error.name === 'AbortError';
}

function boundScore(score: number): number {
  if (!Number.isFinite(score)) return 0;
  return Math.min(100, Math.max(0, score));
}

function buildItems(
  providers: readonly GlobalSearchProvider[],
  providerItems: Map<string, GlobalSearchItem[]>,
  limitPerGroup: number,
): GlobalSearchItem[] {
  const providerOrder = new Map(providers.map((provider, index) => [provider.id, index]));
  const unique = new Map<string, GlobalSearchItem>();

  for (const items of providerItems.values()) {
    for (const item of items) {
      const normalized = { ...item, score: boundScore(item.score) };
      const existing = unique.get(normalized.id);
      if (!existing || normalized.score > existing.score) {
        unique.set(normalized.id, normalized);
      }
    }
  }

  const groupOrder = new Map(GLOBAL_SEARCH_GROUP_ORDER.map((group, index) => [group, index]));
  const counts = new Map<string, number>();
  return [...unique.values()]
    .sort((left, right) => {
      const groupDifference = (groupOrder.get(left.group) ?? 99) - (groupOrder.get(right.group) ?? 99);
      if (groupDifference !== 0) return groupDifference;
      const scoreDifference = right.score - left.score;
      if (scoreDifference !== 0) return scoreDifference;
      const providerDifference =
        (providerOrder.get(left.providerId) ?? 99) - (providerOrder.get(right.providerId) ?? 99);
      if (providerDifference !== 0) return providerDifference;
      return left.title.localeCompare(right.title);
    })
    .filter((item) => {
      const count = counts.get(item.group) ?? 0;
      if (count >= limitPerGroup) return false;
      counts.set(item.group, count + 1);
      return true;
    });
}

export async function runFederatedSearch(
  providers: readonly GlobalSearchProvider[],
  request: GlobalSearchRequest,
  signal: AbortSignal,
  onUpdate: (snapshot: GlobalSearchSnapshot) => void,
): Promise<GlobalSearchSnapshot> {
  const providerItems = new Map<string, GlobalSearchItem[]>();
  const providerDiagnostics = new Map<string, GlobalSearchProviderDiagnostic[]>();
  const providerTruncated = new Map<string, boolean>();
  const providerStatus: Record<string, GlobalSearchProviderStatus> = Object.fromEntries(
    providers.map((provider) => [provider.id, 'loading' as const]),
  );

  const snapshot = (): GlobalSearchSnapshot => ({
    items: buildItems(providers, providerItems, request.limitPerGroup),
    providerStatus: { ...providerStatus },
    diagnostics: [...providerDiagnostics.values()].flat(),
    isSearching: Object.values(providerStatus).some((status) => status === 'loading'),
    truncated: [...providerTruncated.values()].some(Boolean),
  });

  onUpdate(snapshot());

  await Promise.all(providers.map(async (provider) => {
    try {
      const result = await provider.search(request, signal);
      if (signal.aborted) return;
      providerItems.set(provider.id, result.items);
      providerDiagnostics.set(provider.id, result.diagnostics ?? []);
      providerTruncated.set(provider.id, result.truncated === true);
      providerStatus[provider.id] = 'ready';
    } catch (error) {
      if (signal.aborted) return;
      if (isAbortError(error)) {
        providerItems.set(provider.id, []);
        providerDiagnostics.set(provider.id, []);
        providerStatus[provider.id] = 'ready';
      } else {
        providerStatus[provider.id] = 'error';
        providerDiagnostics.set(provider.id, [{
          providerId: provider.id,
          code: 'provider_unavailable',
          message: error instanceof Error ? error.message : String(error),
        }]);
      }
    }
    if (!signal.aborted) onUpdate(snapshot());
  }));

  return snapshot();
}
