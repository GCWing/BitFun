import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { resolveDispatchJobId, resolveSessionDriverId } from '@/flow_chat/session-drivers/resolve';
import { getActiveSurfaceScope } from '@/infrastructure/peer-device/deviceSurface';
import type { LineRange } from '@/shared/editor/LineRange';
import { notificationService } from '@/shared/notification-system';
import { createTab } from '@/shared/utils/tabUtils';
import { dispatchApi } from './dispatchApi';

export function isDispatchFileSession(sessionId: string | undefined): boolean {
  return Boolean(sessionId && resolveSessionDriverId(
    sessionId, flowChatStore.getState().sessions.get(sessionId),
  ) === 'dispatch');
}

/** An immutable target preview: the controller must never probe or watch this path. */
export async function openDispatchSessionFile(
  sessionId: string,
  filePath: string,
  fileName: string,
  lineRange?: LineRange,
): Promise<void> {
  const scope = getActiveSurfaceScope();
  const sessions = flowChatStore.getState().sessions;
  const session = sessions.get(sessionId);
  const jobId = resolveDispatchJobId(sessionId, session, id => sessions.get(id));
  try {
    if (!jobId) throw new Error('This remote session is still connecting to its job. Try opening the file again.');
    const response = await dispatchApi.readFile(jobId, filePath);
    if (!scope.isCurrent()) return;
    const current = flowChatStore.getState();
    if (resolveDispatchJobId(sessionId, current.sessions.get(sessionId), id => current.sessions.get(id)) !== jobId) return;
    if (response.kind !== 'readFile' || response.jobId !== jobId || typeof response.content !== 'string') {
      throw new Error('The target returned an invalid file preview.');
    }
    const previewPath = `dispatch-file://${encodeURIComponent(jobId)}/${encodeURIComponent(response.filePath)}`;
    createTab({
      type: 'code-editor',
      title: fileName,
      data: {
        filePath: previewPath,
        fileName,
        initialContent: response.content,
        readOnly: true,
        jumpToRange: lineRange,
        navigationToken: Date.now(),
      },
      checkDuplicate: true,
      duplicateCheckKey: scope.key(jobId, response.filePath),
      replaceExisting: true,
      mode: 'agent',
      isCurrent: scope.isCurrent,
    });
  } catch (error) {
    if (scope.isCurrent()) {
      notificationService.error(error instanceof Error ? error.message : String(error));
    }
  }
}
