/**
 * Single entry point for opening a BitFun Canvas artifact in a panel tab.
 *
 * Both the Canvas tool card and `bitfun-canvas://` links in assistant markdown
 * route through here, so tab identity, session context resolution, and failure
 * reporting stay identical no matter where the user clicked.
 */

import { createTab } from './tabUtils';
import { parseCanvasArtifactRef } from './canvasArtifactRef';
import { createLogger } from './logger';
import { i18nService } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';

const log = createLogger('CanvasArtifactTab');

export interface CanvasSessionContext {
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

export interface OpenCanvasArtifactTabOptions {
  title?: string;
  /** Inline snapshot from a tool result so the panel renders before its first load round-trip. */
  source?: string;
  status?: string;
  diagnostics?: unknown[];
  /** Overrides the session context that would otherwise be resolved from the reference. */
  sessionContext?: CanvasSessionContext;
  /** Provenance recorded on the tab, mirroring other tool-opened panels. */
  tabSource?: Record<string, unknown>;
  metadata?: Record<string, unknown>;
}

async function resolveSessionContext(sessionId: string): Promise<CanvasSessionContext | null> {
  try {
    const { flowChatStore } = await import('@/flow_chat/store/FlowChatStore');
    const session = flowChatStore.getState().sessions.get(sessionId);
    if (!session) return null;
    return {
      workspacePath: session.workspacePath,
      remoteConnectionId: session.remoteConnectionId,
      remoteSshHost: session.remoteSshHost,
    };
  } catch (error) {
    log.warn('Failed to resolve Canvas session context', { sessionId, error });
    return null;
  }
}

/**
 * Opens the Canvas addressed by `artifactReference`.
 * Returns false (and notifies the user) when the reference cannot be opened.
 */
export async function openCanvasArtifactTab(
  artifactReference: string,
  options: OpenCanvasArtifactTabOptions = {},
): Promise<boolean> {
  const parsed = parseCanvasArtifactRef(artifactReference);
  if (!parsed) {
    log.warn('Refusing to open invalid Canvas artifact reference', { artifactReference });
    notificationService.warning(i18nService.t('components:canvasLink.invalidReference'));
    return false;
  }

  const sessionContext = options.sessionContext
    ?? await resolveSessionContext(parsed.sessionId)
    ?? {};
  if (!options.sessionContext && !sessionContext.workspacePath) {
    // Not fatal: the host falls back to the active workspace, and the panel
    // surfaces a load failure if the canvas does not live there.
    log.info('Opening Canvas without a resolved session workspace', {
      artifactReference,
      sessionId: parsed.sessionId,
    });
  }

  const duplicateCheckKey = `bitfun-canvas-${artifactReference}`;
  createTab({
    type: 'bitfun-canvas',
    title: options.title?.trim() || 'BitFun Canvas',
    data: {
      artifactReference,
      source: options.source,
      status: options.status,
      diagnostics: options.diagnostics,
      workspacePath: sessionContext.workspacePath,
      remoteConnectionId: sessionContext.remoteConnectionId,
      remoteSshHost: sessionContext.remoteSshHost,
      ...(options.tabSource ? { _source: options.tabSource } : {}),
    },
    metadata: {
      duplicateCheckKey,
      artifactReference,
      ...options.metadata,
    },
    checkDuplicate: true,
    duplicateCheckKey,
    replaceExisting: true,
    mode: 'agent',
  });
  return true;
}
