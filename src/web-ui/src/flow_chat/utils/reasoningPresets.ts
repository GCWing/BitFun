const RECENT_PRESETS_STORAGE_KEY = 'openbitfun.reasoning-presets.recent.v1';

function nonEmpty(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

export function getRecentReasoningPreset(modelId: string): string | undefined {
  if (!nonEmpty(modelId) || typeof window === 'undefined') return undefined;
  try {
    const raw = window.localStorage.getItem(RECENT_PRESETS_STORAGE_KEY);
    if (!raw) return undefined;
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const value = parsed[modelId.trim()];
    return nonEmpty(value) ? value : undefined;
  } catch {
    return undefined;
  }
}

export function setRecentReasoningPreset(modelId: string, presetId?: string | null): void {
  if (!nonEmpty(modelId) || typeof window === 'undefined') return;
  try {
    const raw = window.localStorage.getItem(RECENT_PRESETS_STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) as Record<string, unknown> : {};
    const key = modelId.trim();
    if (nonEmpty(presetId)) parsed[key] = presetId.trim();
    else delete parsed[key];
    window.localStorage.setItem(RECENT_PRESETS_STORAGE_KEY, JSON.stringify(parsed));
  } catch {
    // Preferences are best-effort and must not block model selection.
  }
}

