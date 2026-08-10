/**
 * Demo layout
 */

import React from 'react';
import type { ComponentPreview } from '../../types';
import './DemoLayout.css';

interface DemoLayoutProps {
  components: ComponentPreview[];
}

export const DemoLayout: React.FC<DemoLayoutProps> = ({ components }) => {
  return (
    <div className="demo-layout" data-bf-component="component-preview" data-bf-part="demoRoot">
      {components.map((component) => (
        <div key={component.id} className="demo-card" data-bf-component="component-preview" data-bf-part="demoCard">
          <div className="demo-card-header" data-bf-component="component-preview" data-bf-part="demoHeader">
            <h3 className="demo-card-title">{component.name}</h3>
            <p className="demo-card-description">{component.description}</p>
          </div>
          
          <div className="demo-stage" data-bf-component="component-preview" data-bf-part="demoStage">
            <component.component />
          </div>
          
          <div className="demo-card-footer" data-bf-component="component-preview" data-bf-part="demoFooter">
            <span className="demo-id">ID: {component.id}</span>
          </div>
        </div>
      ))}
    </div>
  );
};
