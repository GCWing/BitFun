import { useMemo } from 'react';
import { useLiveAppStore } from './liveAppStore';

export interface RunningLiveAppItem {
  id: string;
  title: string;
  description: string;
  icon: string;
  updatedAt: number;
  overlayId: `live-app:${string}`;
  isRunning: true;
}

function normalizeDescription(description: string, tags: string[]): string {
  const trimmed = description.trim();
  if (trimmed) return trimmed;
  return tags.join(' · ');
}

export function buildRunningLiveAppItems(params: {
  apps: ReturnType<typeof useLiveAppStore.getState>['apps'];
  runningWorkerIds: string[];
}): RunningLiveAppItem[] {
  const { apps, runningWorkerIds } = params;
  if (runningWorkerIds.length === 0 || apps.length === 0) return [];

  const appMap = new Map(apps.map(app => [app.id, app]));
  return runningWorkerIds
    .map(id => appMap.get(id))
    .filter((app): app is NonNullable<typeof app> => Boolean(app))
    .map(app => ({
      id: app.id,
      title: app.name,
      description: normalizeDescription(app.description, app.tags),
      icon: app.icon || 'box',
      updatedAt: app.updated_at,
      overlayId: `live-app:${app.id}` as const,
      isRunning: true as const,
    }))
    .sort((left, right) => right.updatedAt - left.updatedAt);
}

export function useRunningLiveAppItems(): RunningLiveAppItem[] {
  const apps = useLiveAppStore(state => state.apps);
  const runningWorkerIds = useLiveAppStore(state => state.runningWorkerIds);

  return useMemo(
    () => buildRunningLiveAppItems({ apps, runningWorkerIds }),
    [apps, runningWorkerIds]
  );
}

export function resolveActiveRunningLiveAppId(activeOverlay: string | null): string | null {
  if (!activeOverlay?.startsWith('live-app:')) return null;
  return activeOverlay.slice('live-app:'.length) || null;
}
