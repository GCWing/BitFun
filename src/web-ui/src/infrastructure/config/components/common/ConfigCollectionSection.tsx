import React from 'react';
import { ConfigPageSection } from './ConfigPageLayout';
import './ConfigCollectionSection.scss';

export interface ConfigCollectionSectionProps {
  title: string;
  description?: string;
  toolbar?: React.ReactNode;
  filters?: React.ReactNode;
  editor?: React.ReactNode;
  className?: string;
  children: React.ReactNode;
}

export const ConfigCollectionSection: React.FC<ConfigCollectionSectionProps> = ({
  title,
  description,
  toolbar,
  filters,
  editor,
  className = '',
  children,
}) => {
  const hasEditor = Boolean(editor);

  return (
    <ConfigPageSection
      title={title}
      description={description}
      className={`bitfun-config-collection-section ${hasEditor ? 'bitfun-config-collection-section--with-editor' : ''} ${className}`}
    >
      <div className="bitfun-config-collection-section__content" data-bf-component="config" data-bf-part="collectionSection">
        {toolbar && (
          <div className="bitfun-config-collection-section__toolbar" data-bf-component="config" data-bf-part="collectionToolbar">
            {toolbar}
          </div>
        )}
        {editor && (
          <div className="bitfun-config-collection-section__editor" data-bf-component="config" data-bf-part="collectionEditor">
            {editor}
          </div>
        )}
        {filters && (
          <div className="bitfun-config-collection-section__filters" data-bf-component="config" data-bf-part="collectionFilters">
            {filters}
          </div>
        )}
        <div className="bitfun-config-collection-section__list" data-bf-component="config" data-bf-part="collectionList">
          {children}
        </div>
      </div>
    </ConfigPageSection>
  );
};

export default ConfigCollectionSection;
