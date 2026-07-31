import type { AIModelConfig } from '@/infrastructure/config/types';

/**
 * How the target's model catalog relates to this controller's.
 *
 * `unknown` is a real outcome, not an error: the local catalog may not have
 * loaded yet, and claiming parity we have not established would be worse than
 * reporting only what the target advertised.
 */
export type DispatchModelParity = 'match' | 'diverged' | 'unknown';

/**
 * The local model ids a target should end up advertising after a model-config
 * sync.
 *
 * A target's probe lists only the ids it could actually construct a client
 * for, so the comparable local set applies the same two filters the target
 * applies to the configuration it receives: the model must be enabled, and an
 * api-key model must carry a key. A local model that fails either filter can
 * never appear on the target, so counting it as a difference would report a
 * divergence the user cannot resolve by syncing.
 */
export function syncableLocalModelIds(
  models: AIModelConfig[] | null | undefined,
): string[] | null {
  if (!Array.isArray(models)) return null;
  const ids = new Set<string>();
  for (const model of models) {
    const id = model?.id?.trim();
    if (!id || !model.enabled) continue;
    const usesApiKey = !model.auth || model.auth.type === 'api_key';
    if (usesApiKey && !model.api_key?.trim()) continue;
    ids.add(id);
  }
  return Array.from(ids).sort();
}

/**
 * Compare the target's ready model ids against the local ones.
 *
 * Ids are stable across a sync because the sync copies the local catalog
 * verbatim, so set equality is what "same configuration" means here.
 */
export function compareDispatchModels(
  localIds: string[] | null,
  targetIds: string[] | null | undefined,
): DispatchModelParity {
  if (!localIds || !Array.isArray(targetIds)) return 'unknown';
  const target = new Set(targetIds.map(id => id.trim()).filter(Boolean));
  if (target.size !== localIds.length) return 'diverged';
  return localIds.every(id => target.has(id)) ? 'match' : 'diverged';
}
