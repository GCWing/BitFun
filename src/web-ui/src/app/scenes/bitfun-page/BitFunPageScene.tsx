/**
 * BitFun Page scene — Sites-style management surface.
 */
import React, { Suspense, lazy } from 'react';
import './BitFunPageScene.scss';

const BitFunPageView = lazy(() => import('./BitFunPageView'));

const BitFunPageScene: React.FC = () => {
  return (
    <div className="bitfun-page-scene" data-testid="bitfun-page-scene">
      <Suspense fallback={null}>
        <BitFunPageView />
      </Suspense>
    </div>
  );
};

export default BitFunPageScene;
