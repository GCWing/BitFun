import { createTerminalTab } from '@/shared/utils/tabUtils';
import type { ContentResourceScope } from '@/shared/types/contentResource';

interface OpenShellSessionTargetOptions {
  sessionId: string;
  sessionName: string;
  scope?: ContentResourceScope;
}

/** Selected terminals follow the same session-first placement as files. */
export function openShellSessionTarget({ sessionId, sessionName, scope }: OpenShellSessionTargetOptions): void {
  createTerminalTab(sessionId, sessionName, 'project', { scope });
}
