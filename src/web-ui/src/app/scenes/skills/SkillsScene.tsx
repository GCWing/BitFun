/**
 * SkillsScene — Skills scene content.
 * Renders view by activeView from skillsSceneStore.
 * Nav is handled inline via NavPanel SkillsSection (Market / Installed).
 */

import React, { Suspense, lazy } from 'react';
import { useSkillsSceneStore } from './skillsSceneStore';
import './SkillsScene.scss';

const MarketView    = lazy(() => import('./views/MarketView'));
const InstalledView = lazy(() => import('./views/InstalledView'));

const SkillsScene: React.FC = () => {
  const activeView = useSkillsSceneStore((s) => s.activeView);

  const renderView = () => {
    switch (activeView) {
      case 'market':
        return <MarketView />;
      case 'installed-all':
      case 'installed-user':
      case 'installed-project':
      default:
        return <InstalledView />;
    }
  };

  return (
    <div className="bitfun-skills-scene">
      <Suspense fallback={<div className="bitfun-skills-scene__loading" />}>
        {renderView()}
      </Suspense>
    </div>
  );
};

export default SkillsScene;
