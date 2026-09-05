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
      className={`openbitfun-config-collection-section ${hasEditor ? 'openbitfun-config-collection-section--with-editor' : ''} ${className}`}
    >
      <div className="openbitfun-config-collection-section__content" data-openbitfun-component="config" data-openbitfun-part="collectionSection">
        {toolbar && (
          <div className="openbitfun-config-collection-section__toolbar" data-openbitfun-component="config" data-openbitfun-part="collectionToolbar">
            {toolbar}
          </div>
        )}
        {editor && (
          <div className="openbitfun-config-collection-section__editor" data-openbitfun-component="config" data-openbitfun-part="collectionEditor">
            {editor}
          </div>
        )}
        {filters && (
          <div className="openbitfun-config-collection-section__filters" data-openbitfun-component="config" data-openbitfun-part="collectionFilters">
            {filters}
          </div>
        )}
        <div className="openbitfun-config-collection-section__list" data-openbitfun-component="config" data-openbitfun-part="collectionList">
          {children}
        </div>
      </div>
    </ConfigPageSection>
  );
};

export default ConfigCollectionSection;
