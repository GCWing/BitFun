/**
 * Gate for active-session snapshot reconciliation.
 *
 * Reconciliation used to be a Peer Mode concern only, because the local
 * surface was assumed to never miss an event. That stopped being true once
 * devices keep working while the UI renders another one: a turn started on
 * this machine goes on producing events that the routing layer drops for as
 * long as another device is rendered, and the relay protocol has no
 * ACK/replay to recover them.
 *
 * So once this window has switched surface at least once, the rendered
 * surface — local included — heals itself from the host snapshot. A window
 * that never leaves the local device does no extra polling at all.
 */

import { isPeerDeviceModeActive } from './peerModeFlag';

let hasSwitchedSurface = false;

export function markDeviceSurfaceSwitched(): void {
  hasSwitchedSurface = true;
}

export function hasDeviceSurfaceSwitched(): boolean {
  return hasSwitchedSurface;
}

/** Test-only reset; production code never returns to the pristine state. */
export function resetDeviceSurfaceSwitchedForTest(): void {
  hasSwitchedSurface = false;
}

export function isSurfaceReconcileEnabled(): boolean {
  return isPeerDeviceModeActive() || hasSwitchedSurface;
}
