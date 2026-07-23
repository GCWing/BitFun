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
    <div className="demo-layout">
      {components.map((component) => (
        <div key={component.id} className="demo-card">
          <div className="demo-card-header">
            <h3 className="demo-card-title">{component.name}</h3>
            <p className="demo-card-description">{component.description}</p>
          </div>
          
          <div className="demo-stage">
            <component.component />
          </div>
          
          <div className="demo-card-footer">
            <span className="demo-id">ID: {component.id}</span>
          </div>
        </div>
      ))}
    </div>
  );
};