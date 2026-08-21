import { useEffect, useRef, type FC } from 'react';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { createManualTerminalSession } from '@/shared/services/createManualTerminalSession';
import { openShellSessionTarget } from '@/shared/services/openShellSessionTarget';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('TerminalActionBridge');

/** Handles global terminal actions while keeping terminal creation out of NavPanel. */
export const TerminalActionBridge: FC = () => {
  const { workspacePath, workspace } = useCurrentWorkspace();
  const creatingRef = useRef(false);

  useEffect(() => {
    const handleCreate = () => {
      if (creatingRef.current) return;
      creatingRef.current = true;

      void createManualTerminalSession({
        workspacePath,
        connectionId: workspace?.connectionId,
      })
        .then((session) => {
          openShellSessionTarget({ sessionId: session.id, sessionName: session.name });
        })
        .catch((error) => {
          log.error('Failed to create terminal from global action', error);
        })
        .finally(() => {
          creatingRef.current = false;
        });
    };

    window.addEventListener('terminal-create-requested', handleCreate);
    return () => window.removeEventListener('terminal-create-requested', handleCreate);
  }, [workspace?.connectionId, workspacePath]);

  return null;
};

export default TerminalActionBridge;
