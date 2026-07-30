import type { FlowChatPinTurnToTopMode } from '../../events/flowchatNavigation';

export const COMPENSATION_EPSILON_PX = 0.5;

type BottomReservationKind = 'collapse' | 'pin';

interface BottomReservationBase {
  kind: BottomReservationKind;
  px: number;
  floorPx: number;
}

interface CollapseBottomReservation extends BottomReservationBase {
  kind: 'collapse';
}

export interface PinBottomReservation extends BottomReservationBase {
  kind: 'pin';
  mode: FlowChatPinTurnToTopMode;
  targetTurnId: string | null;
}

export interface BottomReservationState {
  collapse: CollapseBottomReservation;
  pin: PinBottomReservation;
}

export interface TurnPinRequestIdentity {
  generation: number;
  sessionId: string;
  turnId: string;
}

export function isTurnPinRequestIdentityCurrent(
  request: TurnPinRequestIdentity,
  current: TurnPinRequestIdentity,
): boolean {
  return (
    request.generation === current.generation &&
    request.sessionId === current.sessionId &&
    request.turnId === current.turnId
  );
}

export function transferCollapseReservationToPin(
  currentState: BottomReservationState,
  nextPinReservation: PinBottomReservation,
): BottomReservationState {
  return {
    ...currentState,
    collapse: {
      ...currentState.collapse,
      px: 0,
      floorPx: 0,
    },
    pin: nextPinReservation,
  };
}

export function transferPinReservationToProtectedCollapse(
  currentState: BottomReservationState,
): BottomReservationState {
  const transferredPx = getReservationTotalPx(currentState.pin);
  return sanitizeBottomReservationState({
    ...currentState,
    collapse: {
      ...currentState.collapse,
      px: currentState.collapse.px + transferredPx,
      floorPx: currentState.collapse.floorPx + transferredPx,
    },
    pin: {
      kind: 'pin',
      px: 0,
      floorPx: 0,
      mode: 'transient',
      targetTurnId: null,
    },
  });
}

export function releasePinReservationForUserNavigation(
  currentState: BottomReservationState,
  options: {
    preserveCurrentRange: boolean;
    ownsElementAnchor: boolean;
  },
): BottomReservationState {
  const isProvisionalStickyPin = (
    currentState.pin.mode === 'sticky-latest' &&
    currentState.pin.floorPx <= COMPENSATION_EPSILON_PX &&
    !options.ownsElementAnchor
  );
  if (options.preserveCurrentRange && !isProvisionalStickyPin) {
    return transferPinReservationToProtectedCollapse(currentState);
  }

  return sanitizeBottomReservationState({
    ...currentState,
    pin: {
      kind: 'pin',
      px: 0,
      floorPx: 0,
      mode: 'transient',
      targetTurnId: null,
    },
  });
}

export function protectCurrentCollapseReservation(
  currentState: BottomReservationState,
): BottomReservationState {
  return sanitizeBottomReservationState({
    ...currentState,
    collapse: {
      ...currentState.collapse,
      floorPx: currentState.collapse.px,
    },
  });
}

export function settleCollapseReservationForPreservedViewport(
  currentState: BottomReservationState,
  geometry: {
    scrollTop: number;
    scrollHeight: number;
    clientHeight: number;
    rangeGuardPx?: number;
  },
): BottomReservationState {
  const currentTotalPx = getReservationTotalPx(currentState.collapse) +
    getReservationTotalPx(currentState.pin);
  const contentHeightWithoutReservation = Math.max(
    0,
    geometry.scrollHeight - currentTotalPx,
  );
  const requiredTotalPx = Math.max(
    getRequiredTotalPxForScrollTop({
      targetScrollTop: geometry.scrollTop,
      contentHeightWithoutReservation,
      clientHeight: geometry.clientHeight,
      rangeGuardPx: geometry.rangeGuardPx,
    }),
  );
  const protectedTotalPx = Math.max(
    getReservationTotalPx(currentState.pin) + currentState.collapse.floorPx,
    requiredTotalPx,
  );
  const settledTotalPx = Math.min(currentTotalPx, protectedTotalPx);
  const settledCollapsePx = Math.max(
    0,
    settledTotalPx - getReservationTotalPx(currentState.pin),
  );

  return sanitizeBottomReservationState({
    ...currentState,
    collapse: {
      ...currentState.collapse,
      px: settledCollapsePx,
      floorPx: settledCollapsePx,
    },
  });
}

export function ensureCollapseReservationForScrollTop(
  currentState: BottomReservationState,
  geometry: {
    targetScrollTop: number;
    scrollHeight: number;
    clientHeight: number;
    rangeGuardPx?: number;
  },
): BottomReservationState {
  const currentPinPx = getReservationTotalPx(currentState.pin);
  const currentCollapsePx = getReservationTotalPx(currentState.collapse);
  const currentTotalPx = currentCollapsePx + currentPinPx;
  const contentHeightWithoutReservation = Math.max(
    0,
    sanitizeReservationPx(geometry.scrollHeight) - currentTotalPx,
  );
  const requiredTotalPx = Math.max(
    currentPinPx,
    getRequiredTotalPxForScrollTop({
      targetScrollTop: geometry.targetScrollTop,
      contentHeightWithoutReservation,
      clientHeight: geometry.clientHeight,
      rangeGuardPx: geometry.rangeGuardPx,
    }),
  );
  const requiredCollapsePx = Math.max(0, requiredTotalPx - currentPinPx);
  const nextCollapsePx = Math.max(currentCollapsePx, requiredCollapsePx);

  return sanitizeBottomReservationState({
    ...currentState,
    collapse: {
      ...currentState.collapse,
      px: nextCollapsePx,
      floorPx: Math.max(currentState.collapse.floorPx, requiredCollapsePx),
    },
  });
}

export function settleRetainedCollapseReservationForAnchor(
  currentState: BottomReservationState,
  geometry: {
    targetScrollTop: number;
    scrollHeight: number;
    clientHeight: number;
    rangeGuardPx?: number;
  },
): BottomReservationState {
  const currentPinPx = getReservationTotalPx(currentState.pin);
  const currentCollapsePx = getReservationTotalPx(currentState.collapse);
  const contentHeightWithoutReservation = Math.max(
    0,
    sanitizeReservationPx(geometry.scrollHeight) - currentPinPx - currentCollapsePx,
  );
  const requiredTotalPx = Math.max(
    currentPinPx,
    getRequiredTotalPxForScrollTop({
      targetScrollTop: geometry.targetScrollTop,
      contentHeightWithoutReservation,
      clientHeight: geometry.clientHeight,
      rangeGuardPx: geometry.rangeGuardPx,
    }),
  );
  const requiredCollapsePx = Math.max(0, requiredTotalPx - currentPinPx);

  return sanitizeBottomReservationState({
    ...currentState,
    collapse: {
      ...currentState.collapse,
      px: requiredCollapsePx,
      floorPx: requiredCollapsePx,
    },
  });
}

export function reconcileUnsignaledShrinkReservation(
  currentState: BottomReservationState,
  fallbackRequiredCollapsePx: number,
): BottomReservationState {
  return sanitizeBottomReservationState({
    ...currentState,
    collapse: {
      ...currentState.collapse,
      px: Math.max(
        currentState.collapse.floorPx,
        sanitizeReservationPx(fallbackRequiredCollapsePx),
      ),
    },
  });
}

export function clampPinReservationPxToViewport(
  reservationPx: number,
  clientHeight: number,
): number {
  return Math.min(
    sanitizeReservationPx(reservationPx),
    sanitizeReservationPx(clientHeight),
  );
}

export function resolveProvisionalStickyPinReservationPx(options: {
  scrollHeight: number;
  clientHeight: number;
  currentPinPx: number;
}): number {
  const currentPinPx = sanitizeReservationPx(options.currentPinPx);
  const effectiveScrollHeight = Math.max(
    0,
    sanitizeReservationPx(options.scrollHeight) - currentPinPx,
  );
  const effectiveMaxScrollTop = Math.max(
    0,
    effectiveScrollHeight - sanitizeReservationPx(options.clientHeight),
  );
  return clampPinReservationPxToViewport(
    Math.max(currentPinPx, effectiveMaxScrollTop),
    options.clientHeight,
  );
}

export function shouldClearExpiredProvisionalStickyPin(options: {
  requestTurnId: string;
  requestPinMode: FlowChatPinTurnToTopMode;
  pinReservation: PinBottomReservation;
  ownsElementAnchor: boolean;
}): boolean {
  return (
    options.requestPinMode === 'sticky-latest' &&
    options.pinReservation.mode === 'sticky-latest' &&
    options.pinReservation.targetTurnId === options.requestTurnId &&
    options.pinReservation.floorPx <= COMPENSATION_EPSILON_PX &&
    !options.ownsElementAnchor
  );
}

export function createInitialBottomReservationState(): BottomReservationState {
  return {
    collapse: {
      kind: 'collapse',
      px: 0,
      floorPx: 0,
    },
    pin: {
      kind: 'pin',
      px: 0,
      floorPx: 0,
      mode: 'transient',
      targetTurnId: null,
    },
  };
}

function sanitizeReservationPx(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}

function getRequiredTotalPxForScrollTop(geometry: {
  targetScrollTop: number;
  contentHeightWithoutReservation: number;
  clientHeight: number;
  rangeGuardPx?: number;
}): number {
  const targetScrollTop = sanitizeReservationPx(geometry.targetScrollTop);
  if (targetScrollTop <= COMPENSATION_EPSILON_PX) {
    return 0;
  }
  return Math.max(
    0,
    targetScrollTop +
      sanitizeReservationPx(geometry.clientHeight) -
      sanitizeReservationPx(geometry.contentHeightWithoutReservation) +
      sanitizeReservationPx(geometry.rangeGuardPx ?? 1),
  );
}

export function sanitizeBottomReservationState(state: BottomReservationState): BottomReservationState {
  const collapsePx = sanitizeReservationPx(state.collapse.px);
  const collapseFloorPx = Math.min(collapsePx, sanitizeReservationPx(state.collapse.floorPx));
  const pinPx = sanitizeReservationPx(state.pin.px);
  const pinFloorPx = Math.min(pinPx, sanitizeReservationPx(state.pin.floorPx));

  return {
    collapse: {
      kind: 'collapse',
      px: collapsePx,
      floorPx: collapseFloorPx,
    },
    pin: {
      kind: 'pin',
      px: pinPx,
      floorPx: pinFloorPx,
      mode: state.pin.mode ?? 'transient',
      targetTurnId: state.pin.targetTurnId ?? null,
    },
  };
}

export function areBottomReservationStatesEqual(
  left: BottomReservationState,
  right: BottomReservationState,
): boolean {
  return (
    Math.abs(left.collapse.px - right.collapse.px) <= COMPENSATION_EPSILON_PX &&
    Math.abs(left.collapse.floorPx - right.collapse.floorPx) <= COMPENSATION_EPSILON_PX &&
    Math.abs(left.pin.px - right.pin.px) <= COMPENSATION_EPSILON_PX &&
    Math.abs(left.pin.floorPx - right.pin.floorPx) <= COMPENSATION_EPSILON_PX &&
    left.pin.mode === right.pin.mode &&
    left.pin.targetTurnId === right.pin.targetTurnId
  );
}

export function getReservationTotalPx(reservation: BottomReservationBase): number {
  return Math.max(0, reservation.px);
}

function getReservationConsumablePx(reservation: BottomReservationBase): number {
  return Math.max(0, reservation.px - reservation.floorPx);
}

export function consumeBottomReservationForContentGrowth(
  state: BottomReservationState,
  amountPx: number,
  consumeStickyPinFloor: boolean,
  preserveCollapseReservation = false,
): BottomReservationState {
  let remaining = Math.max(0, amountPx);

  const collapseConsumablePx = preserveCollapseReservation
    ? 0
    : getReservationConsumablePx(state.collapse);
  const collapseAboveFloorConsumed = Math.min(collapseConsumablePx, remaining);
  remaining -= collapseAboveFloorConsumed;
  const collapseFloorConsumed = preserveCollapseReservation
    ? 0
    : Math.min(state.collapse.floorPx, remaining);
  const collapseConsumed = collapseAboveFloorConsumed + collapseFloorConsumed;
  remaining -= collapseFloorConsumed;

  const pinConsumablePx = getReservationConsumablePx(state.pin);
  const pinConsumed = Math.min(pinConsumablePx, remaining);
  remaining -= pinConsumed;

  const stickyPinFloorConsumed = consumeStickyPinFloor && state.pin.mode === 'sticky-latest'
    ? Math.min(state.pin.floorPx, remaining)
    : 0;

  return sanitizeBottomReservationState({
    collapse: {
      ...state.collapse,
      px: state.collapse.px - collapseConsumed,
      floorPx: state.collapse.floorPx - collapseFloorConsumed,
    },
    pin: {
      ...state.pin,
      px: state.pin.px - pinConsumed - stickyPinFloorConsumed,
      floorPx: state.pin.floorPx - stickyPinFloorConsumed,
    },
  });
}

export function shouldSyncPhysicalBottom(options: {
  viewportGeometryChanged: boolean;
  collapseProtectionActive: boolean;
  wasAtPhysicalBottom: boolean;
  ownsElementAnchor: boolean;
}): boolean {
  return (
    options.viewportGeometryChanged &&
    !options.collapseProtectionActive &&
    options.wasAtPhysicalBottom &&
    !options.ownsElementAnchor
  );
}

export function shouldSuppressFollowingTailNegativeScrollBy(options: {
  requestedTop: number | null;
  isFollowingOutput: boolean;
  isStreamingOutput: boolean;
  wasAtPhysicalBottom: boolean;
}): boolean {
  return (
    options.requestedTop !== null &&
    options.requestedTop < -COMPENSATION_EPSILON_PX &&
    options.isFollowingOutput &&
    options.isStreamingOutput &&
    options.wasAtPhysicalBottom
  );
}

export function getCanceledUnsettledStickyPinGrowthPx(options: {
  pendingGrowthPx: number;
  shrinkPx: number;
  hasActiveCollapseIntent: boolean;
}): number {
  if (options.hasActiveCollapseIntent) {
    return 0;
  }
  return Math.min(
    sanitizeReservationPx(options.pendingGrowthPx),
    sanitizeReservationPx(options.shrinkPx),
  );
}

export function shouldBypassShrinkCompensationInTailFollow(options: {
  isFollowingOutput: boolean;
  isStreamingOutput: boolean;
  hasActiveCollapseIntent: boolean;
}): boolean {
  return (
    options.isFollowingOutput &&
    options.isStreamingOutput &&
    !options.hasActiveCollapseIntent
  );
}

export function shouldPreserveCollapseReservationAfterIntent(options: {
  isFollowingOutput: boolean;
  isStreamingOutput: boolean;
  isPreservingElement: boolean;
  hasProtectedCollapseRange: boolean;
}): boolean {
  return (
    (options.isFollowingOutput && options.isStreamingOutput) ||
    options.isPreservingElement ||
    options.hasProtectedCollapseRange
  );
}

export function resolveAutoCollapseAnchorScrollTop(options: {
  currentScrollTop: number;
  previousStableScrollTop: number;
  reason: string | null | undefined;
  isFollowingOutput: boolean;
  isStreamingOutput: boolean;
}): number {
  if (
    options.reason !== 'auto' ||
    !options.isFollowingOutput ||
    !options.isStreamingOutput
  ) {
    return options.currentScrollTop;
  }
  return Math.max(options.currentScrollTop, options.previousStableScrollTop);
}
