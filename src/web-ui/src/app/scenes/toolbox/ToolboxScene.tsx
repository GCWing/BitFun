/**
 * ToolboxScene — Toolbox tab showing the MiniApp gallery.
 * Opening an app opens a separate scene tab (miniapp:id).
 */
import React, { Suspense, lazy, useEffect } from 'react';
import { useMiniAppList } from './hooks/useMiniAppList';
import { useToolboxStore } from './toolboxStore';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import './ToolboxScene.scss';

const GalleryView = lazy(() => import('./views/GalleryView'));

const ToolboxScene: React.FC = () => {
  useMiniAppList();
  const setRunningWorkerIds = useToolboxStore((s) => s.setRunningWorkerIds);
  const markWorkerRunning = useToolboxStore((s) => s.markWorkerRunning);
  const markWorkerStopped = useToolboxStore((s) => s.markWorkerStopped);

  useEffect(() => {
    miniAppAPI.workerListRunning().then(setRunningWorkerIds).catch(() => {});

    const unlistenRestarted = api.listen<{ id?: string }>('miniapp-worker-restarted', (payload) => {
      if (payload?.id) markWorkerRunning(payload.id);
    });
    const unlistenStopped = api.listen<{ id?: string }>('miniapp-worker-stopped', (payload) => {
      if (payload?.id) markWorkerStopped(payload.id);
    });
    const unlistenDeleted = api.listen<{ id?: string }>('miniapp-deleted', (payload) => {
      if (payload?.id) markWorkerStopped(payload.id);
    });

    return () => {
      unlistenRestarted();
      unlistenStopped();
      unlistenDeleted();
    };
  }, [setRunningWorkerIds, markWorkerRunning, markWorkerStopped]);

  return (
    <div className="toolbox-scene">
      <Suspense fallback={null}>
        <GalleryView />
      </Suspense>
    </div>
  );
};

export default ToolboxScene;
