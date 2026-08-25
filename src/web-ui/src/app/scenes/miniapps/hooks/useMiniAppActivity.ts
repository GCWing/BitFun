import { useMemo } from 'react';
import { useSceneStore } from '@/app/stores/sceneStore';
import { projectMiniAppActivity } from '../miniAppActivity';
import { useMiniAppStore } from '../miniAppStore';

/** Shared UI projection for mounted Runner scenes and background workers. */
export function useMiniAppActivity() {
  const apps = useMiniAppStore((state) => state.apps);
  const runningWorkerIds = useMiniAppStore((state) => state.runningWorkerIds);
  const openTabs = useSceneStore((state) => state.openTabs);

  return useMemo(
    () => projectMiniAppActivity(apps, openTabs, runningWorkerIds),
    [apps, openTabs, runningWorkerIds],
  );
}
