/**
 * CapabilitiesScene — Capabilities scene content. Renders view by activeView from capabilitiesSceneStore.
 * Left nav uses MainNav with inline CapabilitiesSection (sub-agents / skills / mcp).
 */

import React from 'react';
import { useCapabilitiesSceneStore } from './capabilitiesSceneStore';
import { AgentsView, SkillsView, MCPView } from './views';
import './CapabilitiesScene.scss';

const CapabilitiesScene: React.FC = () => {
  const activeView = useCapabilitiesSceneStore((s) => s.activeView);

  const renderView = () => {
    switch (activeView) {
      case 'skills':
        return <SkillsView />;
      case 'mcp':
        return <MCPView />;
      case 'sub-agents':
      default:
        return <AgentsView />;
    }
  };

  return <div className="bitfun-capabilities-scene">{renderView()}</div>;
};

export default CapabilitiesScene;
