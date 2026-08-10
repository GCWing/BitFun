/**
 * Cross-feature event emitted once a permission request is actionable in the
 * UI. ACP requests use this after their matching tool card is available.
 */

export const PERMISSION_REQUEST_NOTIFICATION_EVENT = 'permission:request-ready';

export interface PermissionRequestNotificationEvent {
  /** Opaque request identifier used only for in-memory de-duplication. */
  requestId: string;
  /** Session and round identify requests that should share one notification. */
  sessionId: string;
  roundId?: string;
}
