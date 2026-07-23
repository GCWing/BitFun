/**
 * Column layout
 */

import React, { useRef } from 'react';
import type { ComponentPreview } from '../../types';
import { useI18n } from '@/infrastructure/i18n';
import './ColumnLayout.css';

interface ColumnLayoutProps {
  components: ComponentPreview[];
}

export const ColumnLayout: React.FC<ColumnLayoutProps> = ({ components }) => {
  const { t } = useI18n('components');

  const scrollToComponent = (id: string) => {
    const element = document.getElementById(`component-${id}`);
    if (element) {
      element.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
  };

  return (
    <div className="column-layout">
      <div className="column-nav">
        <div className="column-nav-title">{t('componentLibrary.layouts.quickJump')}</div>
        <div className="column-nav-items">
          {components.map((component) => (
            <button
              key={component.id}
              className="column-nav-item"
              onClick={() => scrollToComponent(component.id)}
              title={component.description}
            >
              {component.name}
            </button>
          ))}
        </div>
      </div>

      <div className="column-content">
        {components.map((component) => (
          <div 
            key={component.id} 
            id={`component-${component.id}`}
            className="column-item"
          >
            <div className="column-item-header">
              <h3 className="column-item-title">{component.name}</h3>
              <p className="column-item-description">{component.description}</p>
            </div>
            
            <div className="column-item-preview">
              <component.component />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};