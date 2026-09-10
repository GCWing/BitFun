import { useEffect, useRef, type FC } from 'react';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { createManualTerminalSession } from '@/shared/services/createManualTerminalSession';
import { openShellSessionTarget } from '@/shared/services/openShellSessionTarget';
import { createLogger } from '@/shared/utils/logger';
import { getActiveSurfaceScope } from '@/infrastructure/peer-device/deviceSurface';
import { isTerminalPathInside, normalizeTerminalPath } from '@/tools/terminal/services/terminalWorkspaceScope';
import { notificationService } from '@/shared/notification-system';
import { useI18n } from '@/infrastructure/i18n';

const log = createLogger('TerminalActionBridge');

/** Handles global terminal actions while keeping terminal creation out of NavPanel. */
export const TerminalActionBridge: FC = () => {
  const { t } = useI18n('common');
  const { workspacePath, workspace } = useCurrentWorkspace();
  const creatingRef = useRef(false);
  const workspaceKey = JSON.stringify([workspace?.id, workspace?.connectionId, workspace?.workspaceKind, workspacePath]);
  const currentWorkspaceKey = useRef(workspaceKey);
  currentWorkspaceKey.current = workspaceKey;

  useEffect(() => {
    let active = true;
    const handleCreate = (event: Event) => {
      const detail = (event as CustomEvent<{
        workingDirectory?: string; workspacePath?: string; surfaceId?: string;
      }>).detail;
      const scope = getActiveSurfaceScope();
      const remote = workspace?.workspaceKind === 'remote';
      if (detail?.surfaceId && detail.surfaceId !== scope.surfaceId) return;
      if (detail?.workspacePath && normalizeTerminalPath(detail.workspacePath, remote) !== normalizeTerminalPath(workspacePath, remote)) return;
      if (detail?.workingDirectory && !isTerminalPathInside(detail.workingDirectory, workspacePath, remote)) return;
      if (remote && !workspace?.connectionId) {
        notificationService.error(t('nav.resources.unavailable'));
        return;
      }
      if (creatingRef.current) return;
      creatingRef.current = true;

      void createManualTerminalSession({
        workspacePath: detail?.workingDirectory ?? workspacePath,
        connectionId: workspace?.connectionId,
      })
        .then((session) => {
          if (!active || !scope.isCurrent() || currentWorkspaceKey.current !== workspaceKey) return;
          openShellSessionTarget({ sessionId: session.id, sessionName: session.name });
        })
        .catch((error) => {
          log.error('Failed to create terminal from global action', error);
          if (active && scope.isCurrent() && currentWorkspaceKey.current === workspaceKey) notificationService.error(t('nav.resources.actionFailed', {
            error: error instanceof Error ? error.message : String(error),
          }));
        })
        .finally(() => {
          creatingRef.current = false;
        });
    };

    window.addEventListener('terminal-create-requested', handleCreate);
    return () => {
      active = false;
      window.removeEventListener('terminal-create-requested', handleCreate);
    };
  }, [t, workspace?.connectionId, workspace?.workspaceKind, workspaceKey, workspacePath]);

  return null;
};

export default TerminalActionBridge;
