export const ASSISTANT_AVATAR_PRESETS = [
  { id: 'signal-pulse', family: 'signal', variant: 1, palette: 'accent' },
  { id: 'signal-wave', family: 'signal', variant: 2, palette: 'violet' },
  { id: 'orbit-nova', family: 'orbit', variant: 1, palette: 'violet' },
  { id: 'orbit-loop', family: 'orbit', variant: 2, palette: 'accent' },
  { id: 'mosaic-grid', family: 'mosaic', variant: 1, palette: 'accent' },
  { id: 'mosaic-stack', family: 'mosaic', variant: 2, palette: 'violet' },
  { id: 'companion-spark', family: 'companion', variant: 1, palette: 'violet' },
  { id: 'companion-calm', family: 'companion', variant: 2, palette: 'accent' },
] as const;

export type AssistantAvatarPreset = typeof ASSISTANT_AVATAR_PRESETS[number];
export type AssistantAvatarPresetId = AssistantAvatarPreset['id'];
export type AssistantAvatarFamily = AssistantAvatarPreset['family'];

const PRESETS_BY_ID = new Map<string, AssistantAvatarPreset>(
  ASSISTANT_AVATAR_PRESETS.map((preset) => [preset.id, preset]),
);

function stableHash(value: string): number {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

export function getAssistantAvatarPreset(value?: string | null): AssistantAvatarPreset | null {
  const normalized = value?.trim();
  return normalized ? PRESETS_BY_ID.get(normalized) ?? null : null;
}

export function resolveAssistantAvatarPreset(
  value?: string | null,
  stableKey?: string | null,
): AssistantAvatarPreset {
  const explicitPreset = getAssistantAvatarPreset(value);
  if (explicitPreset) return explicitPreset;

  const normalizedKey = stableKey?.trim() || 'bitfun-primary-assistant';
  return ASSISTANT_AVATAR_PRESETS[stableHash(normalizedKey) % ASSISTANT_AVATAR_PRESETS.length];
}
