import { flowChatStore } from '../store/FlowChatStore';
import { driverForSession } from './registry';
import type { LineRange } from '@/shared/editor/LineRange';

function sessionFileProvider(sessionId: string | undefined) {
  if (!sessionId) return undefined;
  return driverForSession(sessionId, flowChatStore.getState().sessions.get(sessionId)).fileAccess;
}

export function hasSessionFileProvider(sessionId: string | undefined): boolean {
  return Boolean(sessionFileProvider(sessionId));
}

/** Returns true once the owning transport handles the request, including errors. */
export function openFileThroughSession(
  sessionId: string | undefined,
  filePath: string,
  fileName: string,
  lineRange?: LineRange,
): boolean {
  const provider = sessionFileProvider(sessionId);
  if (!provider || !sessionId) return false;
  void provider.open(sessionId, filePath, fileName, lineRange);
  return true;
}
