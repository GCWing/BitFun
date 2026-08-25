/**
 * Markdown preview page
 */

import React, { useState } from 'react';
import { Markdown } from '@components/Markdown';
import { Button } from '@bitfun/ui';
import { useI18n } from '@/infrastructure/i18n';
import './markdown-preview.css';

export const MarkdownPreview: React.FC = () => {
  const { t } = useI18n('components');
  const getSampleMarkdown = () => t('componentLibrary.markdownPreview.sample');
  const [content, setContent] = useState(() => getSampleMarkdown());
  const [variant, setVariant] = useState<'default' | 'bordered' | 'minimal'>('default');
  const [activeTab, setActiveTab] = useState<'preview' | 'edit'>('preview');

  return (
    <div className="markdown-preview-page" data-bf-component="component-preview" data-bf-part="markdownRoot">
      <header className="markdown-preview-header" data-bf-component="component-preview" data-bf-part="markdownHeader">
        <div className="header-left">
          <h1>{t('componentLibrary.markdownPreview.title')}</h1>
          <span className="badge">{t('componentLibrary.markdownPreview.badge')}</span>
        </div>
        <div className="header-right">
          <Button
            variant="outline"
            size="sm"
            onClick={() => window.location.href = '/preview.html'}
          >
            {t('componentLibrary.markdownPreview.backToLibrary')}
          </Button>
        </div>
      </header>

      <div className="markdown-controls" data-bf-component="component-preview" data-bf-part="markdownControls">
        <div className="control-group">
          <label>{t('componentLibrary.markdownPreview.controls.variantLabel')}</label>
          <div className="button-group">
            <Button
              variant={variant === 'default' ? 'fill' : 'outline'}
              size="sm"
              onClick={() => setVariant('default')}
            >
              {t('componentLibrary.markdownPreview.variants.default')}
            </Button>
            <Button
              variant={variant === 'bordered' ? 'fill' : 'outline'}
              size="sm"
              onClick={() => setVariant('bordered')}
            >
              {t('componentLibrary.markdownPreview.variants.bordered')}
            </Button>
            <Button
              variant={variant === 'minimal' ? 'fill' : 'outline'}
              size="sm"
              onClick={() => setVariant('minimal')}
            >
              {t('componentLibrary.markdownPreview.variants.minimal')}
            </Button>
          </div>
        </div>

        <div className="control-group">
          <label>{t('componentLibrary.markdownPreview.controls.modeLabel')}</label>
          <div className="button-group">
            <Button
              variant={activeTab === 'preview' ? 'fill' : 'outline'}
              size="sm"
              onClick={() => setActiveTab('preview')}
            >
              {t('componentLibrary.markdownPreview.controls.preview')}
            </Button>
            <Button
              variant={activeTab === 'edit' ? 'fill' : 'outline'}
              size="sm"
              onClick={() => setActiveTab('edit')}
            >
              {t('componentLibrary.markdownPreview.controls.edit')}
            </Button>
          </div>
        </div>

        <div className="control-group">
          <Button
            variant="outline"
            size="sm"
            onClick={() => setContent(getSampleMarkdown())}
          >
            {t('componentLibrary.markdownPreview.controls.reset')}
          </Button>
        </div>
      </div>

      <div className="markdown-preview-main">
        {activeTab === 'preview' ? (
          <div className="preview-container" data-bf-component="component-preview" data-bf-part="markdownStage">
            <Markdown content={content} variant={variant} />
          </div>
        ) : (
          <div className="editor-container" data-bf-component="component-preview" data-bf-part="markdownStage">
            <textarea
              className="markdown-editor"
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder={t('componentLibrary.markdownPreview.editorPlaceholder')}
              data-bf-component="component-preview"
              data-bf-part="markdownEditor"
            />
          </div>
        )}
      </div>
    </div>
  );
};
