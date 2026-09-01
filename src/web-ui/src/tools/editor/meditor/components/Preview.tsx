import React from 'react';
import { MarkdownRenderer } from '@/infrastructure/markdown';
import { useI18n } from '@/infrastructure/i18n';
import { splitMarkdownFrontmatter } from '../utils/markdownFrontmatter';
import './Preview.scss';

interface PreviewProps {
  value: string;
  basePath?: string;
}

export const Preview: React.FC<PreviewProps> = ({ value, basePath }) => {
  const { t } = useI18n('tools');
  const frontmatter = splitMarkdownFrontmatter(value);

  return (
    <div className="m-editor-preview" data-bf-component="editor-tool" data-bf-part="meditorPreview">
      <div className="m-editor-preview-content">
        {frontmatter && (
          <section className="m-editor-preview-frontmatter" data-bf-component="editor-tool" data-bf-part="meditorFrontmatter">
            <header className="m-editor-preview-frontmatter__header">
              <span className="m-editor-preview-frontmatter__label">
                {t('editor.meditor.frontmatter.label')}
              </span>
            </header>
            <pre className="m-editor-preview-frontmatter__source">
              <code>{frontmatter.yaml}</code>
            </pre>
          </section>
        )}
        <MarkdownRenderer content={frontmatter?.body ?? value} basePath={basePath} />
      </div>
    </div>
  );
};
