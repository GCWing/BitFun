import type { MiniAppPermissionDiff } from '@/infrastructure/api/service-api/MiniAppAPI';

export function requiresPermissionConfirmation(
  diff: MiniAppPermissionDiff | null | undefined,
): boolean {
  return diff?.high_risk === true;
}
