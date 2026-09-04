import React from 'react';
import ReactMarkdown from 'react-markdown';
import type { Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import rehypeRaw from 'rehype-raw';
import rehypeSanitize from 'rehype-sanitize';
import type { Options as RehypeSanitizeOptions } from 'rehype-sanitize';
import type { Pluggable } from 'unified';
import 'katex/dist/katex.min.css';

interface MarkdownMathRendererProps {
  markdownContent: string;
  components: Components;
  sanitizeSchema: RehypeSanitizeOptions;
  remarkAutolinkComputerFileLinks: Pluggable;
  urlTransform: (value: string) => string;
}

export const MarkdownMathRenderer: React.FC<MarkdownMathRendererProps> = ({
  markdownContent,
  components,
  sanitizeSchema,
  remarkAutolinkComputerFileLinks,
  urlTransform,
}) => (
  <div data-openbitfun-component="markdown" data-openbitfun-part="math">
    <ReactMarkdown
      remarkPlugins={[remarkGfm, remarkMath, remarkAutolinkComputerFileLinks]}
      rehypePlugins={[rehypeRaw, [rehypeSanitize, sanitizeSchema], rehypeKatex]}
      urlTransform={urlTransform}
      components={components}
    >
      {markdownContent}
    </ReactMarkdown>
  </div>
);

export default MarkdownMathRenderer;
