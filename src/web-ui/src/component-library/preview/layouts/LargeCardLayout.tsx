/**
 * Large card layout
 */

import React, { useState } from 'react';
import type { ComponentPreview } from '../../types';
import { useI18n } from '@/infrastructure/i18n';
import './LargeCardLayout.css';

interface LargeCardLayoutProps {
  components: ComponentPreview[];
}

export const LargeCardLayout: React.FC<LargeCardLayoutProps> = ({ components }) => {
  const { t } = useI18n('common');
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());

  const toggleExpand = (id: string) => {
    setExpandedIds((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(id)) {
        newSet.delete(id);
      } else {
        newSet.add(id);
      }
      return newSet;
    });
  };

  return (
    <div className="large-card-layout" data-bf-component="component-preview" data-bf-part="largeCardRoot">
      {components.map((component) => {
        const isExpanded = expandedIds.has(component.id);
        return (
          <div
            key={component.id}
            className={`large-card ${isExpanded ? 'expanded' : ''}`}
            data-bf-component="component-preview"
            data-bf-part="largeCard"
            data-bf-state={isExpanded ? 'expanded' : undefined}
          >
            <div className="large-card-header" data-bf-component="component-preview" data-bf-part="largeCardHeader">
              <div className="large-card-info">
                <h3 className="large-card-title">{component.name}</h3>
                <p className="large-card-description">{component.description}</p>
              </div>
              <button 
                className="expand-button"
                onClick={() => toggleExpand(component.id)}
                title={isExpanded ? t('collapse') : t('expand')}
                data-bf-component="component-preview"
                data-bf-part="expandButton"
                data-bf-state={isExpanded ? 'expanded' : undefined}
              >
                {isExpanded ? t('collapse') : t('expand')}
              </button>
            </div>
            
            <div className="large-card-preview" data-bf-component="component-preview" data-bf-part="largeCardPreview">
              <component.component />
            </div>
          </div>
        );
      })}
    </div>
  );
};
