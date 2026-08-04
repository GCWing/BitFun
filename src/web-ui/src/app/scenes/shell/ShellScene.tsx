import React, { Suspense, lazy } from 'react';
import './ShellScene.scss';

const TerminalScene = lazy(() => import('../terminal/TerminalScene'));

interface ShellSceneProps {
  isActive?: boolean;
}

const ShellScene: React.FC<ShellSceneProps> = ({ isActive = true }) => (
  <div className="bitfun-shell-scene" data-testid="shell-panel" data-bf-scene="shell" data-bf-part="root" data-bf-state={isActive ? 'active' : undefined}>
    <Suspense fallback={<div className="bitfun-shell-scene__loading" data-bf-scene="shell" data-bf-part="loading" />}>
      <TerminalScene isActive={isActive} />
    </Suspense>
  </div>
);

export default ShellScene;
