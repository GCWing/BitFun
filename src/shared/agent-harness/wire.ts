/** The v0 peer/mobile dialect is isolated here; application state uses canonical IDs. */
import { canonicalAgentId, canonicalAgentConfigId, legacyAgentWireId } from './contract.generated';

export type AgentIdentityDialect = 'canonical' | 'legacy';
function identity(id: string, dialect: AgentIdentityDialect, profile = false): string {
  const canonical = profile ? canonicalAgentConfigId(id) : canonicalAgentId(id);
  return dialect === 'canonical' ? canonical
    : profile && canonical === 'Standard' ? 'coding_shared' : legacyAgentWireId(canonical);
}
const agentFields = new Set([
  'agentType', 'agent_type', 'parentAgentType', 'parent_agent_type',
  'modeId', 'mode_id', 'default_mode_id', 'last_mode_id',
  'lastUserDialogAgentType', 'last_user_dialog_agent_type',
  'lastSubmittedAgentType', 'last_submitted_agent_type',
]);
const opaque = new Set(['content', 'prompt', 'arguments', 'args', 'input', 'output', 'metadata', 'userMessageMetadata', 'user_message_metadata', 'value']);

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function mergeProfileRecords(previous: unknown, incoming: unknown): unknown {
  if (previous == null) return incoming;
  if (incoming == null) return previous;
  if (isRecord(previous) && isRecord(incoming)) {
    const merged = new Map(Object.entries(previous));
    for (const [key, value] of Object.entries(incoming)) {
      merged.set(key, merged.has(key) ? mergeProfileRecords(merged.get(key), value) : value);
    }
    return Object.fromEntries(merged);
  }
  if (JSON.stringify(previous) === JSON.stringify(incoming)) return previous;
  throw new Error('Conflicting Agent profile aliases; original configuration was preserved');
}

function translateProfile(value: unknown, dialect: AgentIdentityDialect): unknown {
  if (!isRecord(value) || typeof value.profile_id !== 'string') return value;
  return { ...value, profile_id: identity(value.profile_id, dialect, true) };
}

function translateProfileMap(value: unknown, dialect: AgentIdentityDialect): unknown {
  if (!isRecord(value)) return value;
  const profiles = new Map<string, unknown>();
  for (const [id, config] of Object.entries(value)) {
    const key = identity(id, dialect, true);
    const profile = translateProfile(config, dialect);
    profiles.set(key, profiles.has(key) ? mergeProfileRecords(profiles.get(key), profile) : profile);
  }
  return Object.fromEntries(profiles);
}

function configPath(path: string, dialect: AgentIdentityDialect): string {
  const prefix = 'ai.agent_profiles.';
  if (!path.startsWith(prefix)) return path;
  const [id, ...rest] = path.slice(prefix.length).split('.');
  return prefix + [identity(id, dialect, true), ...rest].join('.');
}

function configValue(path: string, value: unknown, dialect: AgentIdentityDialect): unknown {
  if (path === 'ai.agent_profiles') return translateProfileMap(value, dialect);
  if (['app.flow_chat.default_mode_id', 'app.flow_chat.last_mode_id'].includes(path)) {
    return typeof value === 'string' ? identity(value, dialect) : value;
  }
  if (path.startsWith('ai.agent_profiles.')) {
    const suffix = path.slice('ai.agent_profiles.'.length).split('.').slice(1).join('.');
    if (!suffix) return translateProfile(value, dialect);
    return suffix === 'profile_id' && typeof value === 'string' ? identity(value, dialect, true) : value;
  }
  return ['', 'app', 'app.flow_chat', 'ai'].includes(path)
    ? translateAgentIdentityFields(value, dialect) : value;
}
export function translateAgentIdentityFields<T>(value: T, dialect: AgentIdentityDialect): T {
  if (Array.isArray(value)) return value.map(item => translateAgentIdentityFields(item, dialect)) as T;
  if (!value || typeof value !== 'object') return value;
  const record = value as Record<string, unknown>;
  if (typeof record.source === 'string' && record.source !== 'builtin') return value;
  const catalogEntry = 'toolCount' in record || 'defaultTools' in record;
  return Object.fromEntries(Object.entries(record).map(([key, item]) => {
    const profile = key === 'configProfileId' || key === 'profile_id';
    if ((key === 'agent_profiles' || key === 'ai.agent_profiles') && item && typeof item === 'object' && !Array.isArray(item)) {
      return [key, translateProfileMap(item, dialect)];
    }
    if (typeof item === 'string' && (profile || agentFields.has(key) || (catalogEntry && key === 'id'))) {
      return [key, identity(item, dialect, profile)];
    }
    if (['configProfileMemberModeIds', 'allowedParentAgentIds', 'deniedParentAgentIds'].includes(key) && Array.isArray(item)) {
      return [key, item.map(id => typeof id === 'string' ? identity(id, dialect) : id)];
    }
    return [key, opaque.has(key) ? item : translateAgentIdentityFields(item, dialect)];
  })) as T;
}
export function translateAgentIdentityCommand<T>(command: string, value: T, dialect: AgentIdentityDialect): T {
  const translated = translateAgentIdentityFields(value, dialect);
  if (['set_config', 'get_config', 'get_configs'].includes(command) && isRecord(translated)) {
    const request = isRecord(translated.request) ? translated.request : translated;
    if (typeof request.path === 'string') {
      if (command === 'set_config') request.value = configValue(request.path, request.value, dialect);
      request.path = configPath(request.path, dialect);
    }
    if (command === 'get_configs' && Array.isArray(request.paths)) {
      request.paths = request.paths.map(path => typeof path === 'string' ? configPath(path, dialect) : path);
    }
  }
  return translated;
}

/** Config replies need the request scope because a path read may return a scalar or map. */
export function translateAgentIdentityResponse<T>(command: string, value: T, args: unknown, dialect: AgentIdentityDialect): T {
  if (command === 'get_agent_profile_configs') return translateProfileMap(value, dialect) as T;
  if (command === 'get_config') {
    const request = isRecord(args) && isRecord(args.request) ? args.request : args;
    const path = isRecord(request) && typeof request.path === 'string' ? request.path : '';
    return configValue(path, value, dialect) as T;
  }
  if (command === 'get_configs' && isRecord(value)) {
    return Object.fromEntries(Object.entries(value).map(([path, config]) => [configPath(path, dialect), configValue(path, config, dialect)])) as T;
  }
  return translateAgentIdentityFields(value, dialect);
}
