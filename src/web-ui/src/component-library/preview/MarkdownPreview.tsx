/**
 * Markdown preview page
 */

import React, { useState } from 'react';
import { Markdown } from '@components/Markdown';
import { Button } from '@components/Button';
import { useI18n } from '@/infrastructure/i18n';
import './markdown-preview.css';

export const MarkdownPreview: React.FC = () => {
  const { t } = useI18n('components');
  const getSampleMarkdown = () => t('componentLibrary.markdownPreview.sample');
  const [content, setContent] = useState(() => getSampleMarkdown());
  const [variant, setVariant] = useState<'default' | 'bordered' | 'minimal'>('default');
  const [activeTab, setActiveTab] = useState<'preview' | 'edit'>('preview');

  return (
    <div className="markdown-preview-page">
      <header className="markdown-preview-header">
        <div className="header-left">
          <h1>{t('componentLibrary.markdownPreview.title')}</h1>
          <span className="badge">{t('componentLibrary.markdownPreview.badge')}</span>
        </div>
        <div className="header-right">
          <Button
            variant="ghost"
            size="small"
            onClick={() => window.location.href = '/preview.html'}
          >
            {t('componentLibrary.markdownPreview.backToLibrary')}
          </Button>
        </div>
      </header>

      <div className="markdown-controls">
        <div className="control-group">
          <label>{t('componentLibrary.markdownPreview.controls.variantLabel')}</label>
          <div className="button-group">
            <Button
              variant={variant === 'default' ? 'primary' : 'secondary'}
              size="small"
              onClick={() => setVariant('default')}
            >
              {t('componentLibrary.markdownPreview.variants.default')}
            </Button>
            <Button
              variant={variant === 'bordered' ? 'primary' : 'secondary'}
              size="small"
              onClick={() => setVariant('bordered')}
            >
              {t('componentLibrary.markdownPreview.variants.bordered')}
            </Button>
            <Button
              variant={variant === 'minimal' ? 'primary' : 'secondary'}
              size="small"
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
              variant={activeTab === 'preview' ? 'primary' : 'secondary'}
              size="small"
              onClick={() => setActiveTab('preview')}
            >
              {t('componentLibrary.markdownPreview.controls.preview')}
            </Button>
            <Button
              variant={activeTab === 'edit' ? 'primary' : 'secondary'}
              size="small"
              onClick={() => setActiveTab('edit')}
            >
              {t('componentLibrary.markdownPreview.controls.edit')}
            </Button>
          </div>
        </div>

        <div className="control-group">
          <Button
            variant="ghost"
            size="small"
            onClick={() => setContent(getSampleMarkdown())}
          >
            {t('componentLibrary.markdownPreview.controls.reset')}
          </Button>
        </div>
      </div>

      <div className="markdown-preview-main">
        {activeTab === 'preview' ? (
          <div className="preview-container">
            <Markdown content={content} variant={variant} />
          </div>
        ) : (
          <div className="editor-container">
            <textarea
              className="markdown-editor"
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder={t('componentLibrary.markdownPreview.editorPlaceholder')}
            />
          </div>
        )}
      </div>
    </div>
  );
};