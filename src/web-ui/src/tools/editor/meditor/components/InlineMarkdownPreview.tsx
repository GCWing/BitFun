import React from 'react';
import { MarkdownRenderer } from '@/component-library';

interface InlineMarkdownPreviewProps {
  value: string;
  basePath?: string;
}

export const InlineMarkdownPreview: React.FC<InlineMarkdownPreviewProps> = ({
  value,
  basePath,
}) => {
  return (
    <div className="m-editor-inline-ai-rendered">
      <div className="m-editor-inline-ai-rendered__content" data-testid="md-inline-ai-preview-content">
        <MarkdownRenderer content={value} basePath={basePath} />
      </div>
    </div>
  );
};
