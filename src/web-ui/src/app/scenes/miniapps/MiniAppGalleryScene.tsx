/**
 * MiniAppGalleryScene — Mini App gallery scene.
 * Opening an app opens a separate scene tab (miniapp:id).
 */
import React, { Suspense, lazy } from 'react';
import './MiniAppGalleryScene.scss';

const MiniAppGalleryView = lazy(() => import('./views/MiniAppGalleryView'));

const MiniAppGalleryScene: React.FC = () => {
  return (
    <div className="miniapp-gallery-scene">
      <Suspense fallback={null}>
        <MiniAppGalleryView />
      </Suspense>
    </div>
  );
};

export default MiniAppGalleryScene;
