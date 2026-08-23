import { api } from '@/infrastructure/api/service-api/ApiClient';
import { appearanceService } from '@/infrastructure/appearance';
import { configManager } from '@/infrastructure/config';
import { i18nService, type LocaleId } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import { activateInteractiveCapability } from './interactiveCapabilityActivator';
import { activateProductAction } from './productActionActivator';
import {
  getInteractiveCapability,
  INTERACTIVE_CAPABILITY_CATALOG,
  type InteractiveCapability,
  type InteractiveCapabilityOption,
  type InteractiveCapabilityValueSchema,
} from './interactiveCapabilityCatalog';
import { scoreTextMatch } from './searchMatching';

const log = createLogger('BitFunControlBridge');
const REQUEST_EVENT = 'agentic://bitfun-control-request';
const APPLIED_EVENT = 'agentic://bitfun-control-applied';
const DEFAULT_SEARCH_LIMIT = 20;
const MAX_LIMIT = 50;

type BitFunControlAction = 'list' | 'search' | 'get' | 'open' | 'execute' | 'configure';

export interface BitFunControlRequest {
  requestId: string;
  action: BitFunControlAction;
  query?: string;
  capabilityId?: string;
  itemId?: string;
  operationId?: string;
  optionId?: string;
  arguments?: Record<string, unknown>;
  value?: unknown;
  cursor?: number;
  limit?: number;
}

interface BitFunControlResponse {
  requestId: string;
  success: boolean;
  result?: unknown;
  error?: string;
}

let requestUnlisten: (() => void) | null = null;
let appliedUnlisten: (() => void) | null = null;

interface BitFunControlAppliedEvent {
  capabilityId: string;
  operationId?: string;
  optionId?: string;
  changedPaths: string[];
  value?: unknown;
}

function compactCapability(capability: InteractiveCapability, query = '') {
  const matchingItems = query
    ? capability.items
      .map((item) => ({ item, score: scoreTextMatch(query, [item.titleZh, item.titleEn]) }))
      .filter(({ score }) => score > 0)
      .sort((left, right) => right.score - left.score)
      .slice(0, 5)
      .map(({ item }) => item)
    : [];
  return {
    id: capability.id,
    kind: capability.kind,
    titleZh: capability.titleZh,
    titleEn: capability.titleEn,
    summaryZh: capability.summaryZh,
    summaryEn: capability.summaryEn,
    categoryId: capability.categoryId,
    operationCount: capability.operations.length,
    configurableOptionCount: capability.options.length,
    documentedItemCount: capability.items.length,
    controlCoverage: {
      direct: capability.items.filter(({ control }) => control.kind === 'direct').length,
      delegated: capability.items.filter(({ control }) => control.kind === 'delegate').length,
      interactive: capability.items.filter(({ control }) => control.kind === 'open').length,
    },
    matchedItems: matchingItems,
  };
}

function publicCapability(capability: InteractiveCapability) {
  return {
    id: capability.id,
    kind: capability.kind,
    titleZh: capability.titleZh,
    titleEn: capability.titleEn,
    summaryZh: capability.summaryZh,
    summaryEn: capability.summaryEn,
    categoryId: capability.categoryId,
    highlightsZh: capability.highlightsZh,
    highlightsEn: capability.highlightsEn,
    stepsZh: capability.stepsZh,
    stepsEn: capability.stepsEn,
    agentExamplesZh: capability.agentExamplesZh,
    agentExamplesEn: capability.agentExamplesEn,
    agentControl: capability.agentControl,
    items: capability.items,
    destination: capability.destination,
    operations: capability.operations.map(({ handler: _handler, ...operation }) => operation),
    options: capability.options.map(({ handler: _handler, ...option }) => option),
    docsUrl: capability.docsUrl,
  };
}

function pagination(request: BitFunControlRequest) {
  const cursor = Number.isInteger(request.cursor) && (request.cursor ?? 0) >= 0
    ? request.cursor ?? 0
    : 0;
  const defaultLimit = request.action === 'list' ? MAX_LIMIT : DEFAULT_SEARCH_LIMIT;
  const requestedLimit = Number.isInteger(request.limit) ? request.limit ?? defaultLimit : defaultLimit;
  return { cursor, limit: Math.min(MAX_LIMIT, Math.max(1, requestedLimit)) };
}

export function discoverBitFunCapabilities(request: BitFunControlRequest): unknown {
  const { cursor, limit } = pagination(request);
  const query = request.query?.trim() ?? '';
  const matches = INTERACTIVE_CAPABILITY_CATALOG.capabilities
    .map((capability) => ({
      capability,
      score: request.action === 'search'
        ? scoreTextMatch(query, [
          capability.id,
          capability.titleZh,
          capability.titleEn,
          ...capability.searchTerms,
        ])
        : 1,
    }))
    .filter(({ score }) => score > 0)
    .sort((left, right) => right.score - left.score
      || (right.capability.operations.length + right.capability.options.length)
        - (left.capability.operations.length + left.capability.options.length)
      || left.capability.kind.localeCompare(right.capability.kind)
      || left.capability.id.localeCompare(right.capability.id));
  const items = matches.slice(cursor, cursor + limit)
    .map(({ capability }) => compactCapability(capability, query));
  const nextCursor = cursor + items.length < matches.length ? cursor + items.length : null;
  return {
    catalogDigest: INTERACTIVE_CAPABILITY_CATALOG.digest,
    counts: INTERACTIVE_CAPABILITY_CATALOG.counts,
    totalCount: matches.length,
    cursor,
    nextCursor,
    items,
  };
}

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === 'string' && error.trim()) return error;
  return 'BitFunControl request failed';
}

function valuesEqual(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function readNestedValue(value: unknown, field: string): unknown {
  let current = value;
  for (const segment of field.split('.')) {
    if (!current || typeof current !== 'object' || Array.isArray(current)) return undefined;
    current = (current as Record<string, unknown>)[segment];
  }
  return current;
}

function setNestedValue(
  value: Record<string, unknown>,
  field: string,
  nextValue: unknown,
): Record<string, unknown> {
  const [head, ...tail] = field.split('.');
  if (!head) return value;
  if (tail.length === 0) return { ...value, [head]: nextValue };
  const child = value[head];
  const childRecord = child && typeof child === 'object' && !Array.isArray(child)
    ? child as Record<string, unknown>
    : {};
  return {
    ...value,
    [head]: setNestedValue(childRecord, tail.join('.'), nextValue),
  };
}

function assertValueMatchesSchema(value: unknown, schema: InteractiveCapabilityValueSchema): void {
  if (value === null && schema.nullable) return;
  const typeMatches = (() => {
    switch (schema.type) {
      case 'boolean': return typeof value === 'boolean';
      case 'string': return typeof value === 'string';
      case 'integer': return typeof value === 'number' && Number.isInteger(value);
      case 'number': return typeof value === 'number' && Number.isFinite(value);
      case 'object': return !!value && typeof value === 'object' && !Array.isArray(value);
      case 'array': return Array.isArray(value);
    }
  })();
  if (!typeMatches) throw new Error(`value must match type ${schema.type}`);
  if (schema.enum && !schema.enum.some((candidate) => valuesEqual(candidate, value))) {
    throw new Error(`value must be one of: ${schema.enum.map(String).join(', ')}`);
  }
  if (typeof value === 'number' && schema.minimum !== undefined && value < schema.minimum) {
    throw new Error(`value must be at least ${schema.minimum}`);
  }
  if (typeof value === 'number' && schema.maximum !== undefined && value > schema.maximum) {
    throw new Error(`value must be at most ${schema.maximum}`);
  }
  if (typeof value === 'string' && schema.minLength !== undefined && [...value].length < schema.minLength) {
    throw new Error(`value must contain at least ${schema.minLength} characters`);
  }
  if (typeof value === 'string' && schema.maxLength !== undefined && [...value].length > schema.maxLength) {
    throw new Error(`value must contain at most ${schema.maxLength} characters`);
  }
}

async function currentOptionValue(option: InteractiveCapabilityOption): Promise<unknown> {
  if (option.handler.kind === 'appearanceSelection') {
    await appearanceService.initialize();
    return appearanceService.getSnapshot().selectedAppearanceId;
  }
  if (option.handler.kind === 'language') return i18nService.getCurrentLocale();
  if (option.handler.kind === 'provider') {
    throw new Error(
      `Option ${option.id} must be read through native provider ${option.handler.providerId}`,
    );
  }
  const current = await configManager.getOptionalConfig(option.handler.path);
  if (option.handler.kind === 'config') return current;
  const values = option.handler.fields.map((field) => readNestedValue(current, field));
  if (values.length === 1 || values.every((value) => valuesEqual(value, values[0]))) return values[0];
  return Object.fromEntries(option.handler.fields.map((field, index) => [field, values[index]]));
}

async function inspectCapability(capability: InteractiveCapability): Promise<unknown> {
  const optionValues = Object.fromEntries(await Promise.all(capability.options.map(async (option) => [
    option.id,
    await currentOptionValue(option).catch(() => undefined),
  ] as const)));
  return {
    catalogDigest: INTERACTIVE_CAPABILITY_CATALOG.digest,
    capability: publicCapability(capability),
    currentOptionValues: optionValues,
  };
}

async function configureOption(option: InteractiveCapabilityOption, value: unknown): Promise<void> {
  assertValueMatchesSchema(value, option.valueSchema);
  if (option.handler.kind === 'appearanceSelection') {
    await appearanceService.select(value as string);
    return;
  }
  if (option.handler.kind === 'language') {
    await i18nService.changeLanguage(value as LocaleId);
    return;
  }
  if (option.handler.kind === 'provider') {
    throw new Error(
      `Option ${option.id} must be configured through native provider ${option.handler.providerId}`,
    );
  }
  if (option.handler.kind === 'config') {
    await configManager.setConfig(option.handler.path, value);
    return;
  }
  const loaded = await configManager.getOptionalConfig(option.handler.path);
  const current = loaded as Record<string, unknown> | null | undefined;
  let next = current && typeof current === 'object' && !Array.isArray(current) ? current : {};
  for (const field of option.handler.fields) next = setNestedValue(next, field, value);
  await configManager.setConfig(option.handler.path, next);
}

export async function executeBitFunControlRequest(request: BitFunControlRequest): Promise<unknown> {
  switch (request.action) {
    case 'list':
    case 'search':
      if (request.action === 'search' && !request.query?.trim()) {
        throw new Error('query is required for search');
      }
      return discoverBitFunCapabilities(request);
    case 'get': {
      const capability = request.capabilityId ? getInteractiveCapability(request.capabilityId) : undefined;
      if (!capability) throw new Error(`Unknown BitFun capability: ${request.capabilityId ?? ''}`);
      return inspectCapability(capability);
    }
    case 'open': {
      if (!request.capabilityId) throw new Error('capabilityId is required for open');
      await activateInteractiveCapability(request.capabilityId, { itemId: request.itemId });
      return {
        capabilityId: request.capabilityId,
        itemId: request.itemId,
        opened: true,
        surface: 'desktop',
      };
    }
    case 'execute': {
      const capability = request.capabilityId ? getInteractiveCapability(request.capabilityId) : undefined;
      if (!capability) throw new Error(`Unknown BitFun capability: ${request.capabilityId ?? ''}`);
      const operation = capability.operations.find(({ id }) => id === request.operationId);
      if (!operation) throw new Error(`Unknown operation for ${capability.id}: ${request.operationId ?? ''}`);
      if (operation.handler.kind === 'productAction') {
        await activateProductAction(operation.handler.actionId);
      } else {
        throw new Error(
          `Operation ${capability.id}:${operation.id} must run through native provider ${operation.handler.providerId}`,
        );
      }
      return { capabilityId: capability.id, operationId: operation.id, executed: true };
    }
    case 'configure': {
      const capability = request.capabilityId ? getInteractiveCapability(request.capabilityId) : undefined;
      if (!capability) throw new Error(`Unknown BitFun capability: ${request.capabilityId ?? ''}`);
      const option = capability.options.find(({ id }) => id === request.optionId);
      if (!option) throw new Error(`Unknown option for ${capability.id}: ${request.optionId ?? ''}`);
      if (!Object.prototype.hasOwnProperty.call(request, 'value')) throw new Error('value is required for configure');
      await configureOption(option, request.value);
      const effectiveValue = await currentOptionValue(option);
      return {
        capabilityId: capability.id,
        optionId: option.id,
        configured: true,
        effectiveValue,
      };
    }
    default:
      throw new Error(`Unsupported BitFunControl action: ${String(request.action)}`);
  }
}

async function reportResponse(response: BitFunControlResponse): Promise<void> {
  await api.invoke('report_bitfun_control_result', { request: response });
}

async function handleRequest(request: BitFunControlRequest): Promise<void> {
  try {
    const result = await executeBitFunControlRequest(request);
    await reportResponse({ requestId: request.requestId, success: true, result });
  } catch (error) {
    try {
      await reportResponse({
        requestId: request.requestId,
        success: false,
        error: errorMessage(error),
      });
    } catch (reportError) {
      log.warn('Failed to report BitFunControl result', {
        requestId: request.requestId,
        error: reportError,
      });
    }
  }
}

async function handleAppliedEvent(event: BitFunControlAppliedEvent): Promise<void> {
  try {
    await configManager.applyExternalReload();
    if (event.changedPaths.includes('appearance.selection')) {
      await appearanceService.reconcilePersistedState();
    }
    if (event.changedPaths.includes('app.language') && typeof event.value === 'string') {
      await i18nService.applyPersistedLanguage(event.value as LocaleId);
    }
  } catch (error) {
    log.warn('Failed to synchronize a native BitFunControl mutation into the Web UI', {
      capabilityId: event.capabilityId,
      operationId: event.operationId,
      optionId: event.optionId,
      error,
    });
  }
}

export async function initializeBitFunControlBridge(): Promise<void> {
  if (requestUnlisten) return;
  requestUnlisten = api.listen<BitFunControlRequest>(REQUEST_EVENT, (request) => {
    void handleRequest(request);
  });
  appliedUnlisten = api.listen<BitFunControlAppliedEvent>(APPLIED_EVENT, (event) => {
    void handleAppliedEvent(event);
  });
  try {
    await api.invoke('mark_bitfun_control_surface_ready');
  } catch (error) {
    requestUnlisten();
    requestUnlisten = null;
    appliedUnlisten?.();
    appliedUnlisten = null;
    throw error;
  }
}
