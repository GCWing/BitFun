import type { MiniAppComposerClaim } from '../miniAppStore';

/**
 * Marketplace Agent sessions must stay visible to the user. A runner-owned
 * floating composer satisfies that requirement without navigating away from
 * the MiniApp; otherwise the host falls back to the main session scene.
 */
export function shouldOpenMiniAppAgentRunInMainScene(
  strictRuntime: boolean,
  claim: MiniAppComposerClaim | undefined,
  composerToken: string,
  sessionId: string,
  isActiveMiniApp: boolean,
): boolean {
  if (!strictRuntime) return false;
  if (!isActiveMiniApp || !claim || claim.token !== composerToken) return true;
  return claim.sessionId !== sessionId;
}
