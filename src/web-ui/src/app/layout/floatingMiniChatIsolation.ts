interface MiniAppBubblePermissions {
  agent?: { enabled?: boolean };
}

interface FloatingMiniChatIsolationInput {
  activeMiniAppId: string | null;
  permissions: MiniAppBubblePermissions | undefined;
  hasComposerClaim: boolean;
}

/**
 * Reserves the floating chat surface for an Agentic MiniApp before its iframe
 * finishes claiming the composer, and keeps it reserved if that claim is
 * temporarily released during a reload. Unknown metadata is isolated too:
 * showing a pending surface is safer than flashing an unrelated normal chat.
 */
export function isFloatingMiniChatIsolated({
  activeMiniAppId,
  permissions,
  hasComposerClaim,
}: FloatingMiniChatIsolationInput): boolean {
  if (!activeMiniAppId) return false;
  if (hasComposerClaim) return true;
  if (!permissions) return true;
  // Agent permission is the only capability signal shared by strict
  // marketplace apps and compatibility built-ins. A strict app additionally
  // needs host.chat_composer to claim, but older built-ins such as PPT Live do
  // not declare that host permission even though they own this bubble.
  return permissions.agent?.enabled === true;
}

interface FloatingMiniChatSessionGateInput {
  isolated: boolean;
  claimedSessionId: string | undefined;
  activeSessionId: string | undefined;
}

/**
 * An isolated MiniApp surface may mount the shared ChatPane only when the
 * global session is exactly the session validated and bound by that MiniApp.
 */
export function canRenderFloatingMiniChatSession({
  isolated,
  claimedSessionId,
  activeSessionId,
}: FloatingMiniChatSessionGateInput): boolean {
  if (!isolated) return true;
  return Boolean(claimedSessionId && activeSessionId === claimedSessionId);
}
