import { useEffect, useRef } from 'react';
import { externalSourcesAPI } from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import { useOptionalCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
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
  const { workspacePath } = useOptionalCurrentWorkspace();
  const activeTab = useSettingsStore((state) => state.activeTab);
  const markTabUnseen = useSettingsStore((state) => state.markTabUnseen);
  const acknowledgedScopeRef = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void externalSourcesAPI
      .getEcosystemAwareness(workspacePath)
      .then((unacknowledged) => {
        if (cancelled) return;
        markTabUnseen('external-sources', unacknowledged.length > 0);
      })
      .catch((error) => {
        logger.debug('Could not read external application awareness', { error });
      });
    return () => {
      cancelled = true;
    };
  }, [markTabUnseen, workspacePath]);

  useEffect(() => {
    if (activeTab !== 'external-sources' || acknowledgedScopeRef.current === workspacePath) return;
    acknowledgedScopeRef.current = workspacePath;
    // Clear the dot immediately: the user is looking at the list right now, so
    // waiting for the host round-trip would leave a stale marker on screen.
    markTabUnseen('external-sources', false);
    void externalSourcesAPI
      .getEcosystemAwareness(workspacePath)
      .then((unacknowledged) => (unacknowledged.length > 0
        ? externalSourcesAPI.acknowledgeEcosystems(workspacePath, unacknowledged)
        : undefined))
      .catch((error) => {
        logger.debug('Could not record external application awareness', { error });
      });
  }, [activeTab, markTabUnseen, workspacePath]);
}
