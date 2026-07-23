/**
 * Grid layout
 */

import React from 'react';
import type { ComponentPreview } from '../../types';
import type { LayoutType } from '../../types';
import { useI18n } from '@/infrastructure/i18n';
import './GridLayout.css';

interface GridLayoutProps {
  components: ComponentPreview[];
  columns?: 2 | 3 | 4;
  layoutType?: LayoutType;
}

export const GridLayout: React.FC<GridLayoutProps> = ({ 
  components, 
  columns = 3,
  layoutType = 'default'
}) => {
  const { t } = useI18n('components');
  const gridClass = `grid-layout grid-cols-${columns}`;
  
  return (
    <div className={gridClass}>
      {components.map((component) => (
        <div key={component.id} className="grid-card">
          <div className="grid-card-header">
            <h3 className="grid-card-title">{component.name}</h3>
            <p className="grid-card-description">{component.description}</p>
          </div>
          
          <div className="grid-card-preview">
            <div className="preview-label">{t('componentLibrary.layouts.previewLabel')}</div>
            <div className="preview-canvas">
              <component.component />
            </div>
          </div>
          
          <div className="grid-card-info">
            <dl className="info-list">
              <dt>{t('componentLibrary.layouts.idLabel')}</dt>
              <dd>{component.id}</dd>
            </dl>
          </div>
        </div>
      ))}
    </div>
  );
};