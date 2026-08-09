import { useEffect, useRef } from 'react';
import { externalSourcesAPI } from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import { useOptionalCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { isRemoteWorkspace } from '@/shared/types';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import { createLogger } from '@/shared/utils/logger';

const logger = createLogger('ExternalAppAwareness');

/** Marks the external sources tab when the host found an application the user
 * has never been told about, and clears it once they open the tab.
 *
 * The lookup is lazy on purpose: it only runs while the settings scene is
 * mounted, so a user who never opens settings pays nothing. Failures stay
 * silent because a missing hint is far less harmful than an error toast for
 * something the user did not ask for.
 */
export function useExternalAppAwareness(): void {
  const { workspace, workspacePath } = useOptionalCurrentWorkspace();
  const activeTab = useSettingsStore((state) => state.activeTab);
  const markTabUnseen = useSettingsStore((state) => state.markTabUnseen);
  const acknowledgedScopeRef = useRef<string | null>(null);

  useEffect(() => {
    if (isRemoteWorkspace(workspace)) return;
    let cancelled = false;
    void externalSourcesAPI
      .getEcosystemAwareness(workspacePath)
      .then((unacknowledged) => {
        if (cancelled || acknowledgedScopeRef.current === workspacePath) return;
        markTabUnseen('external-sources', unacknowledged.length > 0);
      })
      .catch((error) => {
        logger.debug('Could not read external application awareness', { error });
      });
    return () => {
      cancelled = true;
    };
  }, [markTabUnseen, workspace, workspacePath]);

  useEffect(() => {
    if (isRemoteWorkspace(workspace)
      || activeTab !== 'external-sources'
      || acknowledgedScopeRef.current === workspacePath) return;
    // Clear the dot immediately while allowing a failed host write to retry.
    markTabUnseen('external-sources', false);
    void externalSourcesAPI
      .getEcosystemAwareness(workspacePath)
      .then((unacknowledged) => (unacknowledged.length > 0
        ? externalSourcesAPI.acknowledgeEcosystems(workspacePath, unacknowledged)
        : undefined))
      .then(() => {
        acknowledgedScopeRef.current = workspacePath;
      })
      .catch((error) => {
        markTabUnseen('external-sources', true);
        logger.debug('Could not record external application awareness', { error });
      });
  }, [activeTab, markTabUnseen, workspace, workspacePath]);
}
