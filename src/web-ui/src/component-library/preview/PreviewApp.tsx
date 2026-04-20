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
import { useTheme } from '@/infrastructure/theme';
import type { ThemeId } from '@/infrastructure/theme';
import './preview.css';

export const PreviewApp: React.FC = () => {
  const { t } = useI18n('components');
  const { themes, themeId, themeType, setTheme, loading } = useTheme();
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
  const themeTypeLabel = themeType === 'light' ? '浅色' : '深色';
  const themeOptions = useMemo<SelectOption[]>(
    () =>
      themes.map((theme) => ({
        label: theme.name,
        value: theme.id,
        description: theme.type === 'light' ? '浅色主题' : '深色主题',
      })),
    [themes]
  );

  return (
    <div className="preview-app">
      <header className="preview-header">
        <div className="preview-logo">
          <h1>{t('componentLibrary.previewApp.title')}</h1>
          <span className="preview-version">v0.2.3</span>
        </div>
        <div className="preview-header-actions">
          <label className="preview-theme-selector">
            <span className="preview-theme-selector__label">主题</span>
            <div className="preview-theme-selector__control">
              <Select
                className="preview-theme-selector__select-component"
                size="small"
                value={themeId ?? ''}
                options={themeOptions}
                onChange={(value) => {
                  if (Array.isArray(value)) {
                    return;
                  }
                  void setTheme(value as ThemeId);
                }}
                disabled={loading || themes.length === 0}
                placement="bottom"
              />
              <span className={`preview-theme-selector__badge preview-theme-selector__badge--${themeType}`}>
                {themeTypeLabel}
              </span>
            </div>
          </label>
        </div>
      </header>

      <div className="preview-container">
        <aside className={`preview-sidebar ${isSidebarCollapsed ? 'preview-sidebar--collapsed' : ''}`}>
          <div className="preview-sidebar-header">
            {!isSidebarCollapsed && (
              <span className="preview-sidebar-title">组件导航</span>
            )}
            <button
              type="button"
              className="preview-sidebar-toggle"
              onClick={() => setIsSidebarCollapsed((prev) => !prev)}
              aria-label={isSidebarCollapsed ? '展开导航' : '收起导航'}
              title={isSidebarCollapsed ? '展开导航' : '收起导航'}
            >
              <span className={`preview-sidebar-toggle__icon ${isSidebarCollapsed ? 'is-collapsed' : ''}`}>
                <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                  <path d="M9 2.5L4.5 7L9 11.5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </span>
            </button>
          </div>
          <nav className="preview-nav">
            {componentRegistry.map((category: ComponentCategory) => (
              <div key={category.id} className="category-section">
                <button
                  className={`category-button ${
                    selectedCategory === category.id ? 'active' : ''
                  }`}
                  onClick={() => handleCategorySelect(category.id)}
                  title={category.name}
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

        <main className={`preview-main ${currentCategory?.layoutType === 'full-page' ? 'preview-main--full' : ''}`}>
          {currentCategory ? (
            <>
              {currentCategory.layoutType !== 'full-page' && (
                <div className="component-header">
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
            <div className="empty-state">
              <p>{t('componentLibrary.previewApp.emptyState')}</p>
            </div>
          )}
        </main>
      </div>
    </div>
  );
};
