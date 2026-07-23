import type { MiniAppCustomizationMetadata } from '@/infrastructure/api/service-api/MiniAppAPI';

export interface MiniAppBuiltinUpdateNotice {
  builtinVersion: number;
  sourceHash: string;
}

export function getMiniAppBuiltinUpdateNotice(
  metadata: MiniAppCustomizationMetadata | null | undefined,
): MiniAppBuiltinUpdateNotice | null {
  if (!metadata?.local_override) {
    return null;
  }
  if (metadata.origin.kind !== 'builtin') {
    return null;
  }
  const builtinVersion = metadata.available_builtin_update?.builtin_version;
  if (typeof builtinVersion !== 'number') {
    return null;
  }
  return {
    builtinVersion,
    sourceHash: metadata.available_builtin_update?.source_hash ?? '',
  };
}
