import React from 'react';
import ReactMarkdown from 'react-markdown';
import rehypeSanitize from 'rehype-sanitize';
import remarkGfm from 'remark-gfm';

export const PrivacyDocument: React.FC<{ content: string }> = ({ content }) => (
  <div className="bitfun-privacy-document" data-testid="privacy-document">
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[rehypeSanitize]}
      components={{ a: ({ children }) => <span>{children}</span> }}
    >
      {content}
    </ReactMarkdown>
  </div>
);
