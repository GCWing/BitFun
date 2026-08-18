import type {
  AcpSessionConfigOption,
  AcpSessionConfigValue,
  AcpSessionModelOption,
} from '@/infrastructure/api/service-api/ACPClientAPI';
import type { ReasoningCatalogProjection } from '@/infrastructure/api/service-api/AIApi';

const FAST_MODE_CONFIG_ID = 'fast-mode';
const FAST_MODE_ON_VALUE = 'on';
const FAST_MODE_OFF_VALUE = 'off';

export interface AcpFastModeState {
  option: AcpSessionConfigOption;
  enabled: boolean;
}

export interface AcpModeState {
  option: Extract<AcpSessionConfigOption, { type: 'select' }>;
  currentValue: string;
  /**
   * Nothing left to pick. An agent that fixes a session's mode says so by
   * publishing only the mode in force — a client cannot know an agent's own
   * rule, and one that allows switching mid-conversation keeps its full list.
   */
  locked: boolean;
}

/**
 * The session mode selector, if the agent offers one.
 *
 * ACP's `mode` category is exactly "the picker that is not the model picker",
 * so this stays generic: dsh-acp publishes its agent presets here, claude-code
 * and codex publish their own permission modes.
 */
export function resolveAcpModeState(
  options: AcpSessionConfigOption[],
): AcpModeState | null {
  const option = options.find(candidate => candidate.category === 'mode');
  if (!option || option.type !== 'select' || option.options.length === 0) return null;
  if (!option.options.some(candidate => candidate.value === option.currentValue)) return null;

  return {
    option,
    currentValue: option.currentValue,
    locked: option.options.length <= 1,
  };
}

export interface AcpReasoningState {
  option: Extract<AcpSessionConfigOption, { type: 'select' }>;
  projection: ReasoningCatalogProjection;
  selectedPreset: string;
}

export function resolveAcpReasoningState(
  options: AcpSessionConfigOption[],
): AcpReasoningState | null {
  const option = options.find(candidate => candidate.category === 'thought_level');
  if (!option || option.type !== 'select' || option.options.length === 0) return null;
  if (!option.options.some(candidate => candidate.value === option.currentValue)) return null;

  return {
    option,
    selectedPreset: option.currentValue,
    projection: {
      status: 'known',
      default_preset: option.currentValue,
      presets: option.options.map((candidate, index) => ({
        id: candidate.value,
        label: candidate.name,
        order: index,
        actions: [{ type: 'effort', value: candidate.value }],
        source: 'adapter_fallback',
      })),
    },
  };
}

export function resolveAcpFastModeState(
  options: AcpSessionConfigOption[],
): AcpFastModeState | null {
  const option = options.find(candidate => candidate.id === FAST_MODE_CONFIG_ID);
  if (!option) return null;

  if (option.type === 'boolean') {
    return { option, enabled: option.currentValue };
  }

  const values = new Set(option.options.map(candidate => candidate.value));
  if (!values.has(FAST_MODE_ON_VALUE) || !values.has(FAST_MODE_OFF_VALUE)) {
    return null;
  }
  if (option.currentValue !== FAST_MODE_ON_VALUE && option.currentValue !== FAST_MODE_OFF_VALUE) {
    return null;
  }

  return {
    option,
    enabled: option.currentValue === FAST_MODE_ON_VALUE,
  };
}

export function buildAcpFastModeValue(
  option: AcpSessionConfigOption,
  enabled: boolean,
): AcpSessionConfigValue | null {
  if (option.type === 'boolean') {
    return { type: 'boolean', value: enabled };
  }

  const value = enabled ? FAST_MODE_ON_VALUE : FAST_MODE_OFF_VALUE;
  return option.options.some(candidate => candidate.value === value)
    ? { type: 'select', value }
    : null;
}

export function getAcpModelProviderName(model: AcpSessionModelOption): string | undefined {
  return normalizedProviderName(model.providerName)
    ?? providerNameFromProviderQualifiedValue(model.description)
    ?? providerNameFromProviderQualifiedValue(model.id);
}

function normalizedProviderName(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

function providerNameFromProviderQualifiedValue(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  if (!trimmed) return undefined;

  const slashIndex = trimmed.indexOf('/');
  if (slashIndex <= 0 || slashIndex === trimmed.length - 1) return undefined;

  const provider = trimmed.slice(0, slashIndex).trim();
  const modelId = trimmed.slice(slashIndex + 1).trim();
  return provider && modelId ? provider : undefined;
}
