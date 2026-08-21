import { useEffect, useRef, useState } from 'react';
import { externalSourcesAPI } from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import { useOptionalCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { isRemoteWorkspace } from '@/shared/types';
import { createLogger } from '@/shared/utils/logger';

const logger = createLogger('ExternalAppAwareness');

/** Reports whether the host found an ecosystem the user has not seen yet, and
 * acknowledges it once the ecosystem compatibility scene becomes active.
 *
 * Failures stay silent because a missing navigation hint is less harmful than
 * an error toast for a background awareness check.
 */
export function useExternalAppAwareness(active: boolean): boolean {
  const { workspace, workspacePath } = useOptionalCurrentWorkspace();
  const remoteWorkspace = isRemoteWorkspace(workspace);
  const [hasUnseen, setHasUnseen] = useState(false);
  const acknowledgedScopeRef = useRef<string | null>(null);
  const currentScopeRef = useRef(workspacePath);
  const activeRef = useRef(active);
  currentScopeRef.current = workspacePath;
  activeRef.current = active;

  useEffect(() => {
    setHasUnseen(false);
    if (remoteWorkspace) return undefined;
    const scope = workspacePath;
    let cancelled = false;
    void externalSourcesAPI
      .getEcosystemAwareness(scope)
      .then((unacknowledged) => {
        if (
          cancelled
          || currentScopeRef.current !== scope
          || activeRef.current
          || acknowledgedScopeRef.current === scope
        ) return;
        setHasUnseen(unacknowledged.length > 0);
      })
      .catch((error) => {
        logger.debug('Could not read external application awareness', { error });
      });
    return () => {
      cancelled = true;
    };
  }, [remoteWorkspace, workspacePath]);

  useEffect(() => {
    if (remoteWorkspace
      || !active
      || acknowledgedScopeRef.current === workspacePath) return;
    const scope = workspacePath;
    // Clear the dot immediately while allowing a failed host write to retry.
    setHasUnseen(false);
    void externalSourcesAPI
      .getEcosystemAwareness(scope)
      .then((unacknowledged) => (unacknowledged.length > 0
        ? externalSourcesAPI.acknowledgeEcosystems(scope, unacknowledged)
        : undefined))
      .then(() => {
        if (currentScopeRef.current !== scope) return;
        acknowledgedScopeRef.current = scope;
        setHasUnseen(false);
      })
      .catch((error) => {
        if (currentScopeRef.current === scope) setHasUnseen(true);
        logger.debug('Could not record external application awareness', { error });
      });
  }, [active, remoteWorkspace, workspacePath]);

  return hasUnseen;
}
