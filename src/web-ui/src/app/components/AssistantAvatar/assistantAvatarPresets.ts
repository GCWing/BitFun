export const ASSISTANT_AVATAR_PRESETS = [
  {
    id: 'claw',
    family: 'claw',
    imageSrc: '/assets/assistant/claw-avatar.webp',
  },
  {
    id: 'claw-orbit',
    family: 'clawOrbit',
    imageSrc: '/assets/assistant/claw-avatar-alt.webp',
  },
] as const;

export type AssistantAvatarPreset = typeof ASSISTANT_AVATAR_PRESETS[number];
export type AssistantAvatarPresetId = AssistantAvatarPreset['id'];
export type AssistantAvatarFamily = AssistantAvatarPreset['family'];

const PRESETS_BY_ID = new Map<string, AssistantAvatarPreset>(
  ASSISTANT_AVATAR_PRESETS.map((preset) => [preset.id, preset]),
);

// Persisted identity documents may still contain one of the retired SVG preset
// ids. Keep reading them, but resolve every legacy id to the new Claw artwork.
const LEGACY_PRESET_ALIASES = new Map<string, AssistantAvatarPresetId>([
  ['signal-pulse', 'claw'],
  ['signal-wave', 'claw-orbit'],
  ['orbit-nova', 'claw-orbit'],
  ['orbit-loop', 'claw'],
  ['mosaic-grid', 'claw'],
  ['mosaic-stack', 'claw-orbit'],
  ['companion-spark', 'claw-orbit'],
  ['companion-calm', 'claw'],
]);

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
  if (!normalized) return null;

  const canonicalId = LEGACY_PRESET_ALIASES.get(normalized) ?? normalized;
  return PRESETS_BY_ID.get(canonicalId) ?? null;
}

export function resolveAssistantAvatarPreset(
  value?: string | null,
  stableKey?: string | null,
): AssistantAvatarPreset {
  const explicitPreset = getAssistantAvatarPreset(value);
  if (explicitPreset) return explicitPreset;

  const normalizedKey = stableKey?.trim();
  if (!normalizedKey) return ASSISTANT_AVATAR_PRESETS[0];

  return ASSISTANT_AVATAR_PRESETS[stableHash(normalizedKey) % ASSISTANT_AVATAR_PRESETS.length];
}
