/**
 * Component preview app
 */

import React, { useMemo, useState } from 'react';
import { componentRegistry } from '../components/registry';
import type { ComponentCategory } from '../types';
import { FullPageLayout, LargeCardLayout, GridLayout, DemoLayout, ColumnLayout } from './layouts';
import { Select } from '@components/Select';
import type { SelectOption } from '@components/Select';
import { useI18n } from '@/infrastructure/i18n';
import { useAppearance } from '@/infrastructure/appearance';
import './preview.css';

export const PreviewApp: React.FC = () => {
  const { t } = useI18n('components');
  const {
    appearances,
    selectedAppearanceId,
    current: appearance,
    select,
    initialized,
  } = useAppearance();
  const appearanceMode = appearance?.mode ?? 'dark';
  const [selectedCategory, setSelectedCategory] = useState<string>(
    componentRegistry[0]?.id || ''
  );
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(false);

  const handleCategorySelect = (categoryId: string) => {
    setSelectedCategory(categoryId);
  };

  const currentCategory = componentRegistry.find(
    (cat) => cat.id === selectedCategory
  );
  const appearanceModeLabel = appearanceMode === 'light'
    ? t('componentLibrary.previewApp.appearanceModeLight')
    : t('componentLibrary.previewApp.appearanceModeDark');
  const appearanceOptions = useMemo<SelectOption[]>(
    () =>
      appearances.map((entry) => ({
        label: entry.name,
        value: entry.id,
        description: entry.mode === 'light'
          ? t('componentLibrary.previewApp.appearanceDescriptionLight')
          : t('componentLibrary.previewApp.appearanceDescriptionDark'),
      })),
    [appearances, t]
  );

  return (
    <div
      className="preview-app"
      data-bf-component="component-preview"
      data-bf-part="root"
    >
      <header className="preview-header" data-bf-component="component-preview" data-bf-part="header">
        <div className="preview-logo" data-bf-component="component-preview" data-bf-part="logo">
          <h1>{t('componentLibrary.previewApp.title')}</h1>
          <span className="preview-version">v0.2.15</span>
        </div>
        <div className="preview-header-actions" data-bf-component="component-preview" data-bf-part="headerActions">
          <label className="preview-appearance-selector" data-bf-component="component-preview" data-bf-part="appearanceSelector">
            <span className="preview-appearance-selector__label">
              {t('componentLibrary.previewApp.appearanceLabel')}
            </span>
            <div className="preview-appearance-selector__control">
              <Select
                className="preview-appearance-selector__select-component"
                size="small"
                value={selectedAppearanceId}
                options={appearanceOptions}
                onChange={(value) => {
                  if (Array.isArray(value)) {
                    return;
                  }
                  void select(String(value));
                }}
                disabled={!initialized || appearances.length === 0}
                placement="bottom"
              />
              <span className={`preview-appearance-selector__badge preview-appearance-selector__badge--${appearanceMode}`}>
                {appearanceModeLabel}
              </span>
            </div>
          </label>
        </div>
      </header>

      <div className="preview-container" data-bf-component="component-preview" data-bf-part="container">
        <aside
          className={`preview-sidebar ${isSidebarCollapsed ? 'preview-sidebar--collapsed' : ''}`}
          data-bf-component="component-preview"
          data-bf-part="sidebar"
          data-bf-state={isSidebarCollapsed ? 'collapsed' : undefined}
        >
          <div className="preview-sidebar-header" data-bf-component="component-preview" data-bf-part="sidebarHeader">
            {!isSidebarCollapsed && (
              <span className="preview-sidebar-title">
                {t('componentLibrary.previewApp.sidebarTitle')}
              </span>
            )}
            <button
              type="button"
              className="preview-sidebar-toggle"
              onClick={() => setIsSidebarCollapsed((prev) => !prev)}
              aria-label={isSidebarCollapsed
                ? t('componentLibrary.previewApp.expandSidebar')
                : t('componentLibrary.previewApp.collapseSidebar')}
              title={isSidebarCollapsed
                ? t('componentLibrary.previewApp.expandSidebar')
                : t('componentLibrary.previewApp.collapseSidebar')}
              data-bf-component="component-preview"
              data-bf-part="sidebarToggle"
              data-bf-state={isSidebarCollapsed ? 'collapsed' : undefined}
            >
              <span className={`preview-sidebar-toggle__icon ${isSidebarCollapsed ? 'is-collapsed' : ''}`}>
                <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                  <path d="M9 2.5L4.5 7L9 11.5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </span>
            </button>
          </div>
          <nav className="preview-nav" data-bf-component="component-preview" data-bf-part="navigation">
            {componentRegistry.map((category: ComponentCategory) => (
              <div key={category.id} className="category-section">
                <button
                  className={`category-button ${
                    selectedCategory === category.id ? 'active' : ''
                  }`}
                  onClick={() => handleCategorySelect(category.id)}
                  title={category.name}
                  data-bf-component="component-preview"
                  data-bf-part="category"
                  data-bf-state={selectedCategory === category.id ? 'active' : undefined}
                >
                  <span className="category-button__dot" />
                  <span className="category-name">
                    {isSidebarCollapsed ? category.name.slice(0, 2) : category.name}
                  </span>
                  {!isSidebarCollapsed && (
                    <span className="component-count">
                      {category.components.length}
                    </span>
                  )}
                </button>
              </div>
            ))}
          </nav>
        </aside>

        <main
          className={`preview-main ${currentCategory?.layoutType === 'full-page' ? 'preview-main--full' : ''}`}
          data-bf-component="component-preview"
          data-bf-part="main"
        >
          {currentCategory ? (
            <>
              {currentCategory.layoutType !== 'full-page' && (
                <div className="component-header" data-bf-component="component-preview" data-bf-part="componentHeader">
                  <h2 className="component-title">{currentCategory.name}</h2>
                  <p className="component-description">
                    {currentCategory.description}
                  </p>
                </div>
              )}

              {currentCategory.layoutType === 'full-page' ? (
                <FullPageLayout components={currentCategory.components} />
              ) : currentCategory.layoutType === 'large-card' ? (
                <LargeCardLayout components={currentCategory.components} />
              ) : currentCategory.layoutType === 'demo' ? (
                <DemoLayout components={currentCategory.components} />
              ) : currentCategory.layoutType === 'column' ? (
                <ColumnLayout components={currentCategory.components} />
              ) : currentCategory.layoutType === 'grid-2' ? (
                <GridLayout components={currentCategory.components} columns={2} />
              ) : currentCategory.layoutType === 'grid-4' ? (
                <GridLayout components={currentCategory.components} columns={4} />
              ) : (
                <GridLayout components={currentCategory.components} columns={3} />
              )}
            </>
          ) : (
            <div className="empty-state" data-bf-component="component-preview" data-bf-part="emptyState">
              <p>{t('componentLibrary.previewApp.emptyState')}</p>
            </div>
          )}
        </main>
      </div>
    </div>
  );
};
