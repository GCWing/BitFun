import React, { useMemo } from 'react';
import ReactMarkdown from 'react-markdown';
import type { Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import rehypeRaw from 'rehype-raw';
import rehypeSanitize from 'rehype-sanitize';
import type { Options as RehypeSanitizeOptions } from 'rehype-sanitize';
import type { Pluggable, PluggableList } from 'unified';
import 'katex/dist/katex.min.css';

interface MarkdownMathRendererProps {
  markdownContent: string;
  components: Components;
  sanitizeSchema: RehypeSanitizeOptions;
  remarkAutolinkComputerFileLinks: Pluggable;
}

export const MarkdownMathRenderer: React.FC<MarkdownMathRendererProps> = ({
  markdownContent,
  components,
  sanitizeSchema,
  remarkAutolinkComputerFileLinks,
}) => {
  const remarkPlugins = useMemo<PluggableList>(
    () => [remarkGfm, remarkMath, remarkAutolinkComputerFileLinks],
    [remarkAutolinkComputerFileLinks],
  );
  const rehypePlugins = useMemo<PluggableList>(
    () => [rehypeRaw, [rehypeSanitize, sanitizeSchema], rehypeKatex],
    [sanitizeSchema],
  );

  return (
    <div data-bf-component="markdown" data-bf-part="math">
      <ReactMarkdown
        remarkPlugins={remarkPlugins}
        rehypePlugins={rehypePlugins}
        components={components}
      >
        {markdownContent}
      </ReactMarkdown>
    </div>
  );
};

export default MarkdownMathRenderer;
