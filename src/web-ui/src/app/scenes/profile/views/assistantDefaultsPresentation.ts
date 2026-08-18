export type AssistantDefaultsStatusFilter =
  | 'all'
  | 'enabled'
  | 'disabled'
  | 'changed'
  | 'unavailable';

export interface AssistantDefaultsFilterableItem {
  name: string;
  description: string;
  source: string;
  originalId: string;
  enabled: boolean;
  defaultEnabled: boolean;
  available: boolean;
}

export interface AssistantDefaultsSummary {
  enabled: number;
  changed: number;
  unavailable: number;
}

export interface McpAvailabilityInput {
  enabled?: boolean;
  status?: string;
}

export function differsFromProductDefault(
  item: Pick<AssistantDefaultsFilterableItem, 'enabled' | 'defaultEnabled'>,
): boolean {
  return item.enabled !== item.defaultEnabled;
}

export function isMcpServerAvailable(server?: McpAvailabilityInput): boolean {
  if (!server) return true;
  if (server.enabled === false) return false;

  const status = server.status?.trim().toLowerCase();
  if (!status) return true;
  return status === 'connected' || status === 'healthy';
}

export function matchesAssistantDefaultsFilter(
  item: AssistantDefaultsFilterableItem,
  filter: AssistantDefaultsStatusFilter,
  query: string,
): boolean {
  const normalizedQuery = query.trim().toLowerCase();
  const matchesQuery = normalizedQuery.length === 0 || [
    item.name,
    item.description,
    item.source,
    item.originalId,
  ].some((value) => value.toLowerCase().includes(normalizedQuery));

  if (!matchesQuery) return false;

  switch (filter) {
    case 'enabled':
      return item.enabled;
    case 'disabled':
      return !item.enabled;
    case 'changed':
      return differsFromProductDefault(item);
    case 'unavailable':
      return !item.available;
    case 'all':
      return true;
  }
}

export function summarizeAssistantDefaults(
  items: AssistantDefaultsFilterableItem[],
): AssistantDefaultsSummary {
  return items.reduce<AssistantDefaultsSummary>((summary, item) => {
    if (item.enabled) summary.enabled += 1;
    if (differsFromProductDefault(item)) summary.changed += 1;
    if (!item.available) summary.unavailable += 1;
    return summary;
  }, { enabled: 0, changed: 0, unavailable: 0 });
}
