import React, { Suspense, lazy } from 'react';
import './ShellScene.scss';

const TerminalScene = lazy(() => import('../terminal/TerminalScene'));

interface ShellSceneProps {
  isActive?: boolean;
}

const ShellScene: React.FC<ShellSceneProps> = ({ isActive = true }) => (
  <div className="openbitfun-shell-scene" data-testid="shell-panel" data-openbitfun-scene="shell" data-openbitfun-part="root" data-openbitfun-state={isActive ? 'active' : undefined}>
    <Suspense fallback={<div className="openbitfun-shell-scene__loading" data-openbitfun-scene="shell" data-openbitfun-part="loading" />}>
      <TerminalScene isActive={isActive} />
    </Suspense>
  </div>
);

export default ShellScene;
