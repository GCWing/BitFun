import type { PeerHostCapabilities } from './PeerConnectionManager';

/**
 * Resolve whether the currently rendered surface can be queried for the
 * read-only tool catalog (`get_all_tools_info`), so the UI skips the invoke
 * when the host can't answer it instead of masking the unsupported error as an
 * empty list.
 *
 * - Local surface: always `true`.
 * - `toolCatalog === true`: advertised by the host → `true`.
 * - `toolCatalog === false`: host explicitly unsupported → `false`.
 * - `toolCatalog === null` (older host that did not advertise the field):
 *   resolve by `hostKind`. An old Desktop always implemented the catalog
 *   (`true`); an old CLI never did (`false`). `hostKind === null` (truly
 *   unknown / still probing) stays optimistic.
 *
 * Mirrors {@link resolveCanCancelTool} for `cancel_tool`. See PR #2428 round 5 #1.
 */
export function canQueryToolCatalogOnSurface(
  peerActive: boolean,
  capabilities: PeerHostCapabilities | null,
): boolean {
  if (!peerActive) {
    return true;
  }
  if (capabilities === null) {
    return true;
  }
  if (capabilities.toolCatalog === true) {
    return true;
  }
  if (capabilities.toolCatalog === false) {
    return false;
  }
  // toolCatalog === null: older host, field absent — decide by host kind.
  return capabilities.hostKind !== 'cli';
}
