export const EXTERNAL_SOURCE_DISCOVERY_POLL_DELAYS_MS = [750, 1_500, 3_000, 5_000] as const;

export function externalSourceDiscoveryPollDelay(attempt: number): number {
  return EXTERNAL_SOURCE_DISCOVERY_POLL_DELAYS_MS[
    Math.min(Math.max(0, attempt), EXTERNAL_SOURCE_DISCOVERY_POLL_DELAYS_MS.length - 1)
  ];
}
