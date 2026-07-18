/**
 * Process-wide Peer Device Mode flag for non-React callers
 * (editor polls, git focus refresh, SSH reconnect, etc.).
 *
 * Updated by PeerDeviceProvider; do not set from product features.
 */

let peerDeviceModeActive = false;

export function setPeerDeviceModeActiveFlag(active: boolean): void {
  peerDeviceModeActive = active;
}

export function isPeerDeviceModeActive(): boolean {
  return peerDeviceModeActive;
}

/** Editor / canvas / search / background-command polls while controlling a peer. */
export const PEER_MODE_FILE_SYNC_POLL_MS = 15_000;
export const PEER_MODE_CANVAS_POLL_MS = 15_000;
export const PEER_MODE_SEARCH_IDLE_POLL_MS = 30_000;
export const PEER_MODE_SEARCH_ACTIVE_POLL_MS = 5_000;
export const PEER_MODE_BACKGROUND_COMMAND_POLL_MS = 5_000;
